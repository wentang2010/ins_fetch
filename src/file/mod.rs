use anyhow::{anyhow, Result};
use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;

use crate::const_var::ENV_MODE;

pub fn read_file(file_name: &str) -> Result<String> {
    let mut path_name = "./config/dev/";
    if ENV_MODE == "production" {
        path_name = "./config/pro/";
    }

    let full_path = format!("{}{}", path_name, file_name);

    let mut f = File::open(full_path)?;

    let mut c = String::new();

    f.read_to_string(&mut c)?;

    Ok(c)
}

pub fn write_file(file_name: &str, cnt: &str) -> Result<()> {
    let mut path_name = "./config/dev/";
    if ENV_MODE == "production" {
        path_name = "./config/pro/";
    }

    let full_path = format!("{}{}", path_name, file_name);
    let mut f = File::create(full_path)?;
    f.write_all(cnt.as_bytes())?;

    Ok(())
}

pub fn add_to_log_v1(msg: String, truncate: bool, file_name: String) -> Result<()> {
    let mut v = OpenOptions::new();
    let opts = if truncate {
        v.read(true).write(true).truncate(true)
    } else {
        v.read(true).create(true).append(true)
    };
    let mut f = opts
        .open(format!("log/{}", file_name))
        .map_err(|e| anyhow!("create log file failed {e:?}"))?;
    let fmt = "%Y-%m-%d %H:%M:%S";
    let now = Local::now().format(fmt);

    writeln!(&mut f, "{}---->{}", now, msg)?;
    Ok(())
}

pub fn add_to_log(msg: String, truncate: bool) -> Result<()> {
    add_to_log_v1(msg, truncate, "log.txt".to_string())
}

#[cfg(test)]
mod tests {
    use crate::common::util;
    use chrono::Local;
    use serde_json::{json, Value};
    use std::fs;
    use std::time::SystemTime;

    use super::*;

    #[test]
    fn test() {
        let c = read_file("conf").unwrap();
        let v: Value = serde_json::from_str(&c).unwrap();
        println!("cnt {}", v["start"]);
        // write_file("conf", "{start: 1}").unwrap();
        // add_to_log("ok llhell".to_string(), false).unwrap();
        // add_to_log("sdfsfdsdfsdf".to_string(), false).unwrap();
    }

    #[test]
    fn test1() {
        let v = json!({
            "start" :1
        });
        write_file("conf", v.to_string().as_str()).unwrap();
        let c = read_file("conf").unwrap();
        println!("cnt {}", c);
        let v = json!({
            "start" :999999999999999999u64
        });
        write_file("conf", v.to_string().as_str()).unwrap();
        let c = read_file("conf").unwrap();
        println!("cnt {}", c);
        // add_to_log("ok llhell".to_string(), false).unwrap();
        // add_to_log("sdfsfdsdfsdf".to_string(), false).unwrap();
    }

    // #[tokio::main]
    // #[test]
    // async fn download_img() -> Result<()> {
    //     let path =
    //         std::path::Path::new("cmc_img/SUI").join(format!("{}.{}", "0X2::SUI::SUI", "png"));
    //     std::fs::File::create(&path)?;
    //     // let url = "https://s2.coinmarketcap.com/static/img/coins/64x64/1027.png";

    //     // download_file(url, "cmc_img", vec!["test".to_string()]).await?;
    //     Ok(())
    // }
}
