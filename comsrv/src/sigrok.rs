use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};

use bitvec::order::Lsb0;
use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;
use tokio::task;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum SigrokError {
    #[error("Unexpected output: {code}")]
    UnexpectedOutput {
        code: i32,
        stdout: String,
        stderr: String,
    },
    #[error("Invalid Output")]
    InvalidOutput,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Acquire {
    Time(f32),
    Samples(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokRequest {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    channels: Vec<String>,
    acquire: Acquire,
    sample_rate: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    tsample: f64,
    length: usize,
    channels: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Device {
    addr: String,
    desc: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SigrokResponse {
    Data(Data),
    Devices(Vec<Device>),
}

pub async fn read(device: String, req: SigrokRequest) -> crate::Result<SigrokResponse> {
    let ret = task::spawn_blocking(|| do_read(device, req))
        .await
        .map_err(|_| crate::Error::Disconnected)?;
    ret.map(SigrokResponse::Data)
}

fn run_command(args: &[&str]) -> crate::Result<String> {
    let mut cmd = Command::new("sigrok-cli");
    cmd.stderr(Stdio::piped()).stdout(Stdio::piped());
    for arg in args {
        cmd.arg(arg);
    }
    let child = cmd.spawn().map_err(crate::Error::io)?;
    let output = child.wait_with_output().map_err(crate::Error::io)?;
    let stdout = String::from_utf8(output.stdout).map_err(crate::Error::DecodeError)?;
    let stderr = String::from_utf8(output.stderr).map_err(crate::Error::DecodeError)?;
    let code = output.status.code().unwrap_or(-1);
    if code != 0 {
        let se = SigrokError::UnexpectedOutput {
            code,
            stdout,
            stderr,
        };
        return Err(crate::Error::Sigrok(se));
    }
    Ok(stdout)
}

pub async fn list() -> crate::Result<SigrokResponse> {
    task::spawn_blocking(|| do_list())
        .await
        .map_err(|_| crate::Error::Disconnected)?
}

fn do_list() -> crate::Result<SigrokResponse> {
    let stdout = run_command(&["--scan"])?;
    let mut ret = Vec::new();
    // skip 2, first line is boilerplate, second one is demo
    for line in stdout.split("\n").skip(2) {
        let line = line.trim();
        if line == "" {
            continue;
        }
        let mut parts = line.split("-").map(|x| x.trim());
        let device = Device {
            addr: parts
                .next()
                .ok_or(crate::Error::Sigrok(SigrokError::InvalidOutput))?
                .to_string(),
            desc: parts
                .next()
                .ok_or(crate::Error::Sigrok(SigrokError::InvalidOutput))?
                .to_string(),
        };
        ret.push(device)
    }
    Ok(SigrokResponse::Devices(ret))
}

fn do_read(device: String, req: SigrokRequest) -> crate::Result<Data> {
    let mut args = vec!["-d", &device];
    let channels = req.channels.join(",");
    if req.channels.len() > 0 {
        args.push("--channels");
        args.push(&channels);
    }
    args.push("--config");
    let sample_rate = format!("samplerate={}", req.sample_rate);
    args.push(&sample_rate);
    let acq = match req.acquire {
        Acquire::Time(t) => {
            args.push("--time");
            format!("{}s", t)
        }
        Acquire::Samples(samples) => {
            args.push("--samples");
            format!("{}", samples)
        }
    };
    args.push(&acq);

    let mut tempfile = NamedTempFile::new()?;
    let fpath = tempfile.path().to_str().unwrap();
    args.push("--output-format");
    args.push("csv:label=channel:header=false");
    args.push("--output-file");
    args.push(fpath);
    run_command(&args)?;

    let mut csv = String::new();
    tempfile.read_to_string(&mut csv)?;
    let (channels, length) = parse_csv(csv)?;
    Ok(Data {
        tsample: 1.0 / (req.sample_rate as f64),
        length,
        channels,
    })
}

pub fn parse_csv(data: String) -> crate::Result<(HashMap<String, Vec<u8>>, usize)> {
    let mut ret = HashMap::new();
    let mut cols = Vec::new();
    let mut line_iter = data.split("\n");
    let head = line_iter.next();
    if head.is_none() {
        return Err(crate::Error::Sigrok(SigrokError::InvalidOutput));
    }
    let head = head.unwrap();
    let mut channels: Vec<_> = head.split(",").map(|x| x.to_string()).collect();
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
        for (k, v) in line.split(",").enumerate() {
            if k >= channels.len() {
                return Err(crate::Error::Sigrok(SigrokError::InvalidOutput));
            }
            let v = match v {
                "0" => false,
                "1" => true,
                _ => return Err(crate::Error::Sigrok(SigrokError::InvalidOutput)),
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
