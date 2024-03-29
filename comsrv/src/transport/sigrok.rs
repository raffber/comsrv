use std::collections::HashMap;
use std::process::{Command, Stdio};

use anyhow::anyhow;
use bitvec::order::Lsb0;
use bitvec::vec::BitVec;
use comsrv_protocol::{SigrokAcquire, SigrokData, SigrokDevice, SigrokRequest, SigrokResponse};
use tokio::task;

pub async fn read(device: &str, req: SigrokRequest) -> crate::Result<SigrokResponse> {
    let device = device.to_string();
    let ret = task::spawn_blocking(|| do_read(device, req))
        .await
        .map_err(|x| crate::Error::internal(anyhow!(x)))?;
    ret.map(SigrokResponse::Data)
}

fn run_command(args: &[&str]) -> crate::Result<String> {
    let mut cmd = Command::new("sigrok-cli");
    cmd.stderr(Stdio::piped()).stdout(Stdio::piped());
    for arg in args {
        cmd.arg(arg);
    }
    let child = cmd.spawn().map_err(crate::Error::transport)?;
    let output = child.wait_with_output().map_err(crate::Error::transport)?;
    let stdout = String::from_utf8(output.stdout).map_err(|_| crate::Error::transport(anyhow!("Decode Error")))?;
    let stderr = String::from_utf8(output.stderr).map_err(|_| crate::Error::transport(anyhow!("Decode Error")))?;
    let code = output.status.code().unwrap_or(-1);
    if code != 0 {
        let err = anyhow!("Process failed {}: stdout: `{}`, stderr: `{}`", code, stdout, stderr);
        return Err(crate::Error::transport(err));
    }
    Ok(stdout)
}

pub async fn list() -> crate::Result<SigrokResponse> {
    task::spawn_blocking(do_list)
        .await
        .map_err(|_| crate::Error::internal(anyhow!("Disconnected")))?
}

fn do_list() -> crate::Result<SigrokResponse> {
    let stdout = run_command(&["--scan"])?;
    let mut ret = Vec::new();
    // skip 2, first line is boilerplate, second one is demo
    for line in stdout.split('\n').skip(2) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('-').map(|x| x.trim());
        let addr = parts
            .next()
            .ok_or_else(|| crate::Error::transport(anyhow!("Invalid Output")))?
            .to_string();
        let addr = format!("sigrok::{}", addr);
        let desc = parts
            .next()
            .ok_or_else(|| crate::Error::transport(anyhow!("Invalid Output")))?
            .to_string();
        let device = SigrokDevice { addr, desc };
        ret.push(device)
    }
    Ok(SigrokResponse::Devices(ret))
}

fn do_read(device: String, req: SigrokRequest) -> crate::Result<SigrokData> {
    let mut args = vec!["-d", &device];
    let channels = req.channels.join(",");
    if !req.channels.is_empty() {
        args.push("--channels");
        args.push(&channels);
    }
    args.push("--config");
    let sample_rate = format!("samplerate={}", req.sample_rate);
    args.push(&sample_rate);
    let acq = match req.acquire {
        SigrokAcquire::Time(t) => {
            args.push("--time");
            format!("{}s", t)
        }
        SigrokAcquire::Samples(samples) => {
            args.push("--samples");
            format!("{}", samples)
        }
    };
    args.push(&acq);
    args.push("--output-format");
    args.push("csv:label=channel:header=false");
    let csv = run_command(&args)?;

    let (channels, length) = parse_csv(csv)?;
    Ok(SigrokData {
        tsample: 1.0 / (req.sample_rate as f64),
        length,
        channels,
    })
}

pub fn parse_csv(data: String) -> crate::Result<(HashMap<String, Vec<u8>>, usize)> {
    let mut ret = HashMap::new();
    let mut cols = Vec::new();
    let mut line_iter = data.split('\n');
    let head = line_iter.next();
    if head.is_none() {
        return Err(crate::Error::transport(anyhow!("Invalid Output")));
    }
    let head = head.unwrap();
    let mut channels: Vec<_> = head.split(',').map(|x| x.to_string()).collect();
    for _ in &channels {
        let vec: BitVec<Lsb0, u8> = BitVec::new();
        cols.push(vec);
    }

    let mut len = 0;
    for line in line_iter {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        for (k, v) in line.split(',').enumerate() {
            if k >= channels.len() {
                return Err(crate::Error::transport(anyhow!("Invalid Output")));
            }
            let v = match v {
                "0" => false,
                "1" => true,
                _ => return Err(crate::Error::transport(anyhow!("Invalid Output"))),
            };
            cols[k].push(v)
        }
        len += 1;
    }
    for (k, ch) in channels.drain(..).enumerate() {
        let data = cols[k].as_bitslice().as_slice().to_vec();
        ret.insert(ch, data);
    }
    Ok((ret, len))
}
