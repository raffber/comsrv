pub fn cobs_pack(data: &[u8]) -> Vec<u8> {
    let mut zero_idx: i32 = -1;
    let mut ret = Vec::new();
    for (k, x) in data.iter().enumerate() {
        let k = k as u8;
        if *x != 0 {
            continue;
        }
        ret.push(((k as i32) - zero_idx) as u8);
        ret.extend_from_slice(&data[(zero_idx + 1) as usize..k as usize]);
        zero_idx = k as i32;
    }
    ret.push((data.len() as i32 - zero_idx) as u8);
    ret.extend_from_slice(&data[(zero_idx + 1) as usize..]);
    ret.push(0);
    ret
}

pub fn cobs_unpack(data: &[u8]) -> Option<Vec<u8>> {
    let mut ret = Vec::new();
    let mut zero_idx = 0;
    for (k, x) in data.iter().enumerate() {
        if *x == 0 {
            if ret.len() == 0 {
                return Some(Vec::new());
            }
            return Some(ret[1..].to_vec());
        } else if k == zero_idx {
            zero_idx = k + *x as usize;
            ret.push(0);
        } else {
            ret.push(*x);
        }
    }
    None
}
