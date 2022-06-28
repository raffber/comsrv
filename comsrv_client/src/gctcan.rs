use std::time::Duration;

use comsrv_protocol::{GctMessage, SysCtrlType};

use crate::can::{CanBus, Message};

#[derive(Clone, Debug)]
pub struct MonitorIndex {
    group_index: u8,
    reading_index: u8,
}

impl MonitorIndex {
    pub fn new(group_index: u8, reading_index: u8) -> anyhow::Result<Self> {
        if group_index > 31 {
            anyhow::bail!("Invalid group index {}. Must be <= 31.", group_index);
        }
        if reading_index > 63 {
            anyhow::bail!("Invalid reading index {}. Must be <= 63.", reading_index);
        }
        Ok(MonitorIndex {
            group_index,
            reading_index,
        })
    }

    fn group_index(&self) -> u8 {
        self.group_index
    }

    fn reading_index(&self) -> u8 {
        self.reading_index
    }
}

#[derive(Clone, Debug)]
pub struct MonitorValue {
    pub index: MonitorIndex,
    pub value: Vec<u8>,
}

impl MonitorValue {
    pub fn new(index: MonitorIndex, value: Vec<u8>) -> Self {
        Self { index, value }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NodeId(u8);

impl NodeId {
    pub fn new(node_id: u8) -> anyhow::Result<NodeId> {
        if node_id >= 0x7F || node_id == 0 {
            anyhow::bail!("Invalid node_id: {}. Must be > 0 and < 0x7F", node_id);
        }
        Ok(NodeId(node_id))
    }

    pub fn broadcast_address() -> u8 {
        0x7F
    }
}

#[derive(Clone)]
pub struct GctCanDevice {
    bus: CanBus,
    controller_node_id: NodeId,
}

impl GctCanDevice {
    pub fn new(bus: CanBus, controller_node_id: NodeId) -> Self {
        Self {
            bus,
            controller_node_id,
        }
    }

    pub fn can_bus(&self) -> &CanBus {
        &self.bus
    }

    pub fn can_bus_mut(&mut self) -> &mut CanBus {
        &mut self.bus
    }

    pub fn controller_node_id(&self) -> NodeId {
        self.controller_node_id
    }

    pub async fn monitor_request_no_timeout(
        &mut self,
        destination: NodeId,
        group: u8,
        readings: &[u8],
    ) -> crate::Result<Vec<MonitorValue>> {
        let mut request = 0u64;
        if group > 31 {
            return Err("Invalid group index".into());
        }
        for x in readings {
            if *x > 63 {
                return Err("Invalid reading index".into());
            }
            request |= 1_u64 << x;
        }

        let mut subscription = self
            .bus
            .subscribe({
                let group = group;
                let destination = destination;
                move |x| match x {
                    Message::Gct(GctMessage::MonitoringData {
                        src,
                        group_idx,
                        reading_idx,
                        data,
                    }) => {
                        if src == destination.0 && group_idx == group {
                            Some(MonitorValue {
                                index: MonitorIndex {
                                    group_index: group_idx,
                                    reading_index: reading_idx,
                                },
                                value: data,
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            })
            .await;

        self.bus
            .clone()
            .send(crate::can::Message::Gct(GctMessage::MonitoringRequest {
                src: self.controller_node_id.0,
                dst: destination.0,
                group_idx: group,
                readings: request,
            }))
            .await?;

        let mut ret: Vec<Option<MonitorValue>> = Vec::with_capacity(64);
        for _ in 0..64 {
            ret.push(None);
        }
        let mut filled = 0;
        while let Some(value) = subscription.recv().await {
            let idx = value.index.reading_index;
            if !readings.contains(&idx) {
                continue;
            }
            if ret[idx as usize].is_none() {
                filled += 1;
            }
            ret[idx as usize] = Some(value);
            if filled == readings.len() {
                break;
            }
        }
        let ret = ret.iter().filter_map(|x| x.clone()).collect();
        Ok(ret)
    }

    pub async fn monitor_request(
        &mut self,
        destination: NodeId,
        group: u8,
        readings: &[u8],
        timeout: Duration,
    ) -> crate::Result<Vec<MonitorValue>> {
        match tokio::time::timeout(
            timeout,
            self.monitor_request_no_timeout(destination, group, readings),
        )
        .await
        {
            Ok(x) => x,
            Err(_) => Err(crate::Error::Timeout),
        }
    }

    pub async fn sysctrl_write(
        &mut self,
        cmd: u16,
        destination: NodeId,
        data: Vec<u8>,
    ) -> crate::Result<()> {
        let msg = GctMessage::SysCtrl {
            src: self.controller_node_id.0,
            dst: destination.0,
            cmd,
            tp: comsrv_protocol::SysCtrlType::None,
            data,
        };
        self.bus.send(Message::Gct(msg)).await
    }

    pub async fn sysctrl_read_no_timeout(
        &mut self,
        cmd: u16,
        destination: NodeId,
    ) -> crate::Result<Vec<u8>> {
        self.sysctrl_write_read_no_timeout(cmd, destination, vec![])
            .await
    }

    pub async fn sysctrl_write_read_no_timeout(
        &mut self,
        cmd: u16,
        destination: NodeId,
        data: Vec<u8>,
    ) -> crate::Result<Vec<u8>> {
        let msg = GctMessage::SysCtrl {
            src: self.controller_node_id.0,
            dst: destination.0,
            cmd,
            tp: comsrv_protocol::SysCtrlType::Query,
            data,
        };
        let mut subscription = self
            .bus
            .subscribe({
                let ref_cmd = cmd;
                let destination = destination;
                move |x| match x {
                    Message::Gct(GctMessage::SysCtrl {
                        src,
                        dst: _,
                        cmd,
                        tp,
                        data,
                    }) => {
                        if src == destination.0 && cmd == ref_cmd && tp == SysCtrlType::Value {
                            Some(data)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            })
            .await;
        self.bus.clone().send(Message::Gct(msg)).await?;
        if let Some(x) = subscription.recv().await {
            Ok(x)
        } else {
            Err(crate::Error::EndpointHangUp)
        }
    }

    pub async fn sysctrl_read(
        &mut self,
        cmd: u16,
        destination: NodeId,
        timeout: Duration,
    ) -> crate::Result<Vec<u8>> {
        match tokio::time::timeout(timeout, self.sysctrl_read_no_timeout(cmd, destination)).await {
            Ok(x) => x,
            Err(_) => Err(crate::Error::Timeout),
        }
    }

    pub async fn sysctrl_write_read(
        &mut self,
        cmd: u16,
        destination: NodeId,
        data: Vec<u8>,
        timeout: Duration,
    ) -> crate::Result<Vec<u8>> {
        match tokio::time::timeout(
            timeout,
            self.sysctrl_write_read_no_timeout(cmd, destination, data),
        )
        .await
        {
            Ok(x) => x,
            Err(_) => Err(crate::Error::Timeout),
        }
    }
}
