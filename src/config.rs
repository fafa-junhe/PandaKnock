use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// 1. 定义需要保存到文件中的数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub ports_str: String,
    pub close_ports_str: String, // 新增：关闭端口序列
    pub delay: u64,
}

// 2. 为 Config 实现一个默认值
impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            ports_str: "5000, 6000, 7000".to_string(),
            close_ports_str: "7000, 6000, 5000".to_string(), // 新增：默认的关闭序列
            delay: 1000,
        }
    }
}

// 3. 找到配置文件的路径
fn get_config_path() -> Option<PathBuf> {
    // 使用 directories-next 找到一个安全的、跨平台的位置
    // 例如:
    // - Linux: /home/alice/.config/PandaKnocker
    // - macOS: /Users/Alice/Library/Application Support/com.PandaDeKnocker.PandaKnocker
    // - Windows: C:\Users\Alice\AppData\Roaming\PandaDeKnocker\PandaKnocker\config
    directories_next::ProjectDirs::from("com", "PandaDeKnocker", "PandaKnocker")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

// 4. 加载配置的函数
// 如果文件存在且有效，则加载。否则，创建一个默认配置并保存它。
pub fn load_or_create() -> Config {
    if let Some(path) = get_config_path() {
        if path.exists() {
            // 文件存在，尝试读取和解析
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str(&content) {
                    println!("成功从 {} 加载配置", path.display());
                    return config;
                }
            }
            // 如果读取或解析失败，则打印错误并使用默认值
            eprintln!("配置文件 {} 损坏，使用默认配置。", path.display());
        }

        // 文件不存在或损坏，使用默认值并尝试保存
        let default_config = Config::default();
        if let Err(e) = save(&default_config) {
            eprintln!("无法创建初始配置文件：{}", e);
        } else {
            println!("已在 {} 创建默认配置文件", path.display());
        }
        default_config
    } else {
        // 如果连配置目录都找不到，只能使用内存中的默认值
        eprintln!("无法找到配置目录，将仅在内存中使用默认配置。");
        Config::default()
    }
}

// 5. 保存配置的函数
// 这个函数会返回一个 Result，以便我们可以向用户显示成功或失败的消息。
pub fn save(config: &Config) -> Result<(), String> {
    let path = get_config_path().ok_or_else(|| "无法找到配置目录".to_string())?;

    // 确保父目录存在
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    // 将配置序列化为格式化的 JSON 字符串
    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;

    // 将内容写入文件
    fs::write(&path, content).map_err(|e| format!("写入配置文件失败: {}", e))?;

    println!("配置已成功保存到 {}", path.display());
    Ok(())
}
