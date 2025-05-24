mod config;
mod gamepad_controller;
mod gui;
mod logger;
mod embedded_font;

use eframe::egui;
use gui::GamepadMouseApp;
use log::{info, error, LevelFilter};
use logger::initialize_enhanced_logging;

fn main() -> Result<(), eframe::Error> {
    // 初始化增强型日志系统
    if let Err(e) = initialize_enhanced_logging(Some(LevelFilter::Info)) {
        eprintln!("警告：无法初始化增强型日志系统：{}，尝试使用简单日志系统", e);
        // 回退到简单日志系统
        if let Err(e) = logger::initialize_simple_logging(Some(LevelFilter::Info)) {
            eprintln!("警告：无法初始化日志系统：{}", e);
        }
    }
    info!("游戏手柄鼠标控制工具 v0.2.0 已启动");
    info!("增强型日志系统已激活，日志将保存至本地文件");
    
    // 获取操作系统信息
    let os_info = format!(
        "系统信息: {}, {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    info!("{}", os_info);
    
    // 捕获并记录panic信息，防止程序意外关闭
    std::panic::set_hook(Box::new(|panic_info| {
        if let Some(location) = panic_info.location() {
            error!("程序发生panic：{} (位于 {}:{})",
                   panic_info.to_string(),
                   location.file(),
                   location.line());
        } else {
            error!("程序发生panic：{}", panic_info.to_string());
        }
        eprintln!("程序遇到了一个错误。错误信息已记录到日志文件中。请重新启动应用程序。");
    }));
    
    // 设置环境选项
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 650.0])
            .with_min_inner_size([400.0, 500.0])
            .with_position([300.0, 200.0]),
        default_theme: eframe::Theme::Dark,  // 默认使用深色主题
        follow_system_theme: true,  // 但也跟随系统主题
        ..Default::default()
    };
    
    // 启动GUI应用
    info!("正在启动GUI应用");
    eframe::run_native(
        "游戏手柄鼠标控制器",
        options,
        Box::new(|cc| Box::new(GamepadMouseApp::new(cc))),
    )
}
