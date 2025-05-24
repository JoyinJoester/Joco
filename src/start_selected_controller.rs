/// 启动选中的手柄控制器
pub fn start_selected_controller(&mut self) {
    // 获取当前选中的手柄
    if let Some(gamepad) = self.get_selected_gamepad() {
        info!("正在尝试连接手柄: {} (id: {:?})", gamepad.1, gamepad.0);
        
        // 尝试初始化Gilrs
        match Gilrs::new() {
            Ok(gilrs) => {
                // 检查手柄是否还存在
                if gilrs.gamepad(gamepad.0).is_connected() {
                    info!("手柄已连接，开始初始化控制器");
                    
                    // 创建控制器实例，使用try_catch模式处理可能的失败
                    let controller_result = std::panic::catch_unwind(|| {
                        GamepadController::new(
                            gilrs,
                            gamepad.0,
                            self.config.clone(),
                        )
                    });
                    
                    match controller_result {
                        Ok(controller) => {
                            // 检查控制器是否正常初始化并运行
                            if controller.is_running() {
                                // 保存控制器引用
                                self.controller = Some(Arc::new(Mutex::new(controller)));
                                self.gamepad_name = gamepad.1.clone();
                                self.status_message = "已连接，控制器运行中".to_string();
                                self.status_color = Color32::GREEN;
                                self.active = true;
                                self.tray_tooltip = format!("游戏手柄鼠标控制器 - {}", self.gamepad_name);
                                info!("手柄控制器启动成功");
                            } else {
                                self.status_message = "控制器初始化失败，未能启动".to_string();
                                self.status_color = Color32::RED;
                                self.active = false;
                                info!("控制器初始化成功但未能启动");
                            }
                        },
                        Err(e) => {
                            self.status_message = "控制器初始化失败".to_string();
                            self.status_color = Color32::RED;
                            self.active = false;
                            error!("控制器初始化失败: {:?}", e);
                        }
                    }
                } else {
                    self.status_message = format!("手柄已断开连接: {}", gamepad.1);
                    self.status_color = Color32::RED;
                    self.active = false;
                    info!("手柄已断开连接: {}", gamepad.1);
                }
            },
            Err(err) => {
                self.status_message = format!("无法初始化手柄系统: {}", err);
                self.status_color = Color32::RED;
                self.active = false;
                error!("无法初始化手柄系统: {}", err);
            }
        }
    } else {
        self.status_message = "未选择手柄".to_string();
        self.status_color = Color32::RED;
        info!("未选择手柄，无法启动控制器");
    }
}
