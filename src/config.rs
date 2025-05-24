use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use log::{info, error};

/// 应用配置结构体
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    // 鼠标控制设置
    pub mouse_sensitivity: f32,
    pub dead_zone: f32,
    pub scroll_sensitivity: f32, 
    pub mouse_acceleration: f32,
    
    // 按键映射 - 可以根据需要扩展
    pub left_click_button: String,
    pub right_click_button: String,
    pub middle_click_button: String,
    pub double_click_button: String,   // 双击按钮
    
    // 摇杆配置
    pub invert_x_axis: bool,           // 是否反转X轴
    pub invert_y_axis: bool,           // 是否反转Y轴
    pub use_left_stick_for_mouse: bool, // 是否使用左摇杆控制鼠标（默认右摇杆）
    
    // 操作模式
    pub precision_mode_button: String,  // 精确模式按钮（降低灵敏度）
    pub turbo_mode_button: String,      // 加速模式按钮（提高灵敏度）
    
    // 其他设置
    pub start_minimized: bool,
    pub start_with_system: bool,
    pub show_notification: bool,        // 显示通知
}

impl Default for Config {    fn default() -> Self {
        Self {            // 默认设置 - 调整为更灵敏的值
            mouse_sensitivity: 60.0, // 极大幅度提高左摇杆鼠标控制灵敏度
            dead_zone: 0.03,         // 进一步降低死区以提高响应性
            scroll_sensitivity: 3.0,  // 较低的滚轮灵敏度，但确保功能正常
            mouse_acceleration: 1.4,  // 提高加速度曲线，使鼠标移动显著更敏感
            
            // 默认按键映射
            left_click_button: "South".to_string(),  // A按钮
            right_click_button: "East".to_string(),  // B按钮
            middle_click_button: "West".to_string(), // X按钮
            double_click_button: "North".to_string(), // Y按钮
              // 摇杆配置
            invert_x_axis: false, 
            invert_y_axis: false,
            use_left_stick_for_mouse: true, // 使用左摇杆控制鼠标光标，右摇杆控制滚轮
            
            // 操作模式
            precision_mode_button: "LeftTrigger2".to_string(), // 左肩键
            turbo_mode_button: "RightTrigger2".to_string(),    // 右肩键
            
            // 其他设置
            start_minimized: false,
            start_with_system: false,
            show_notification: true,
        }
    }
}

impl Config {
    /// 尝试从文件中加载配置，如果失败则使用默认配置
    pub fn load() -> Self {
        let config_path = Config::get_config_path();
        
        if let Ok(config_str) = fs::read_to_string(&config_path) {
            match serde_json::from_str(&config_str) {
                Ok(config) => {
                    info!("配置已从 {:?} 成功加载", config_path);
                    return config;
                }
                Err(e) => {
                    error!("解析配置文件失败: {}", e);
                }
            }
        }
        
        // 如果加载失败，则使用默认配置
        let default_config = Config::default();
        info!("使用默认配置");
        default_config
    }
    
    /// 保存配置到文件
    pub fn save(&self) -> Result<(), String> {
        let config_path = Config::get_config_path();
        
        // 确保存在父目录
        if let Some(parent) = Path::new(&config_path).parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return Err(format!("无法创建配置目录: {}", e));
                }
            }
        }
        
        // 将配置序列化为JSON并写入文件
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&config_path, json) {
                    return Err(format!("无法写入配置文件: {}", e));
                }
                info!("配置已保存到 {:?}", config_path);
                Ok(())
            }
            Err(e) => Err(format!("配置序列化失败: {}", e)),
        }
    }
    
    /// 获取配置文件路径
    fn get_config_path() -> String {
        let mut path = dirs::config_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .to_string_lossy()
            .to_string();
        
        path.push_str("/gamepad-mouse-control/config.json");
        path
    }
}
