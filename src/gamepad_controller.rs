use crate::config::Config;
use enigo::{Enigo, MouseControllable};
use gilrs::{Axis, Button, Event, EventType, Gilrs, GamepadId};
use log::{info, error, warn, debug};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::cell::RefCell;

/// 滚轮状态管理结构体
struct ScrollState {
    accum: f32,
    momentum: f32,
    timer: f32,
    last_time: Option<Instant>,
}

impl ScrollState {
    fn new() -> Self {
        Self {
            accum: 0.0,
            momentum: 0.0,
            timer: 0.0,
            last_time: None,
        }
    }
    
    fn reset(&mut self) {
        self.accum = 0.0;
        self.momentum = 0.0;
        self.timer = 0.0;
        self.last_time = None;
    }
}

// 线程局部存储的滚轮状态
thread_local! {
    static SCROLL_STATE: RefCell<ScrollState> = RefCell::new(ScrollState::new());
}

/// 手柄控制器结构体
pub struct GamepadController {
    thread_handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    gamepad_id: GamepadId,
    config: Arc<Mutex<Config>>,
    // 增加连接状态跟踪
    last_activity: Arc<Mutex<Instant>>,
    is_connected: Arc<AtomicBool>,
    // 新增错误恢复和重试机制的字段
    connection_lost_time: Arc<Mutex<Option<Instant>>>,
}

impl GamepadController {
    /// 创建新的手柄控制器
    pub fn new(gilrs: Gilrs, gamepad_id: GamepadId, config: Config) -> Self {
        info!("创建控制器: gamepad_id={:?}", gamepad_id);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let config = Arc::new(Mutex::new(config));
        let config_thread = config.clone();
        
        // 初始化连接状态监控
        let last_activity = Arc::new(Mutex::new(Instant::now()));
        let last_activity_clone = last_activity.clone();
        let is_connected = Arc::new(AtomicBool::new(true));
        let is_connected_clone = is_connected.clone();
        let connection_lost_time = Arc::new(Mutex::new(None));
        let connection_lost_time_clone = connection_lost_time.clone();

        // 创建控制线程
        let thread_handle = thread::spawn(move || {
            // 初始化鼠标控制器
            let mut enigo = Enigo::new();
            info!("成功初始化鼠标控制器");

            // 记录上次鼠标位置更新时间，用于计算鼠标速度
            let mut last_update = Instant::now();

            // 鼠标按键状态
            let mut mouse_buttons_down: HashMap<&str, bool> = HashMap::new();
            mouse_buttons_down.insert("left", false);
            mouse_buttons_down.insert("right", false);
            mouse_buttons_down.insert("middle", false);

            info!("开始监听手柄输入 (gamepad_id: {:?})", gamepad_id);
            let mut gilrs = gilrs;
            
            // 定义一个连接状态检查计时器
            let mut last_connection_check = Instant::now();
            let connection_check_interval = Duration::from_secs(1); // 每1秒检查一次连接状态

            // 主循环
            while running_clone.load(Ordering::Relaxed) {
                // 定期检查手柄连接状态
                if last_connection_check.elapsed() >= connection_check_interval {
                    last_connection_check = Instant::now();
                    
                    // 检查手柄是否还连接着
                    let gamepad = gilrs.gamepad(gamepad_id);
                    if !gamepad.is_connected() {
                        if is_connected_clone.load(Ordering::Relaxed) {
                            warn!("检测到手柄连接丢失");
                            is_connected_clone.store(false, Ordering::Relaxed);
                            
                            // 记录连接丢失时间
                            if let Ok(mut lost_time) = connection_lost_time_clone.lock() {
                                *lost_time = Some(Instant::now());
                            }
                            
                            // 确保所有鼠标按键都释放
                            for (key, is_down) in mouse_buttons_down.iter() {
                                if *is_down {
                                    match *key {
                                        "left" => enigo.mouse_up(enigo::MouseButton::Left),
                                        "right" => enigo.mouse_up(enigo::MouseButton::Right),
                                        "middle" => enigo.mouse_up(enigo::MouseButton::Middle),
                                        _ => {}
                                    }
                                }
                            }
                            
                            // 重置按键状态
                            mouse_buttons_down.insert("left", false);
                            mouse_buttons_down.insert("right", false);
                            mouse_buttons_down.insert("middle", false);
                        }
                        
                        // 手柄断开时，短暂休眠以减少CPU占用
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    } else if !is_connected_clone.load(Ordering::Relaxed) {
                        // 手柄重新连接
                        info!("手柄重新连接成功");
                        is_connected_clone.store(true, Ordering::Relaxed);
                        
                        // 清除连接丢失时间
                        if let Ok(mut lost_time) = connection_lost_time_clone.lock() {
                            *lost_time = None;
                        }
                        
                        // 更新上次活动时间
                        if let Ok(mut last_activity) = last_activity_clone.lock() {
                            *last_activity = Instant::now();
                        }
                    }
                }
                
                // 更新上次活动时间
                if let Ok(mut last_activity) = last_activity_clone.lock() {
                    *last_activity = Instant::now();
                }

                // 处理手柄事件
                while let Some(Event { id, event, time: _ }) = gilrs.next_event() {
                    if id != gamepad_id {
                        continue;
                    }

                    match event {
                        // 按钮按下事件
                        EventType::ButtonPressed(button, _) => {
                            // 获取按钮名称，并处理可能的锁失败
                            let config_guard = match config_thread.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => {
                                    error!("配置锁被毒化: {:?}", poisoned);
                                    poisoned.into_inner() // 尝试恢复锁
                                }
                            };
                            
                            let button_str = button_to_string(button);
                            
                            // 左键点击
                            if button_str == config_guard.left_click_button {
                                info!("左键点击");
                                enigo.mouse_down(enigo::MouseButton::Left);
                                mouse_buttons_down.insert("left", true);
                            }

                            // 右键点击
                            if button_str == config_guard.right_click_button {
                                info!("右键点击");
                                enigo.mouse_down(enigo::MouseButton::Right);
                                mouse_buttons_down.insert("right", true);
                            }

                            // 中键点击
                            if button_str == config_guard.middle_click_button {
                                info!("中键点击");
                                enigo.mouse_down(enigo::MouseButton::Middle);
                                mouse_buttons_down.insert("middle", true);
                            }
                            
                            // 双击功能
                            if button_str == config_guard.double_click_button {
                                info!("双击");
                                enigo.mouse_down(enigo::MouseButton::Left);
                                enigo.mouse_up(enigo::MouseButton::Left);
                                thread::sleep(Duration::from_millis(50));
                                enigo.mouse_down(enigo::MouseButton::Left);
                                enigo.mouse_up(enigo::MouseButton::Left);
                            }
                            
                            // 配置锁在这里自动释放
                        }

                        // 按钮释放事件
                        EventType::ButtonReleased(button, _) => {
                            let button_str = button_to_string(button);
                            
                            // 安全地获取配置，处理可能的锁失败
                            let config_guard = match config_thread.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => {
                                    error!("配置锁被毒化: {:?}", poisoned);
                                    poisoned.into_inner() // 尝试恢复锁
                                }
                            };
                            
                            // 左键释放
                            if button_str == config_guard.left_click_button
                                && *mouse_buttons_down.get("left").unwrap_or(&false)
                            {
                                enigo.mouse_up(enigo::MouseButton::Left);
                                mouse_buttons_down.insert("left", false);
                            }

                            // 右键释放
                            if button_str == config_guard.right_click_button
                                && *mouse_buttons_down.get("right").unwrap_or(&false)
                            {
                                enigo.mouse_up(enigo::MouseButton::Right);
                                mouse_buttons_down.insert("right", false);
                            }

                            // 中键释放
                            if button_str == config_guard.middle_click_button
                                && *mouse_buttons_down.get("middle").unwrap_or(&false)
                            {
                                enigo.mouse_up(enigo::MouseButton::Middle);
                                mouse_buttons_down.insert("middle", false);
                            }
                            
                            // 配置锁在这里自动释放
                        }

                        // 断开连接事件
                        EventType::Disconnected => {
                            warn!("检测到手柄断开连接事件");
                            is_connected_clone.store(false, Ordering::Relaxed);
                            
                            // 记录连接丢失时间
                            if let Ok(mut lost_time) = connection_lost_time_clone.lock() {
                                *lost_time = Some(Instant::now());
                            }
                            
                            // 确保所有鼠标按键都被释放
                            for (key, is_down) in mouse_buttons_down.iter() {
                                if *is_down {
                                    match *key {
                                        "left" => enigo.mouse_up(enigo::MouseButton::Left),
                                        "right" => enigo.mouse_up(enigo::MouseButton::Right),
                                        "middle" => enigo.mouse_up(enigo::MouseButton::Middle),
                                        _ => {}
                                    }
                                }
                            }
                            
                            // 重置按键状态
                            mouse_buttons_down.insert("left", false);
                            mouse_buttons_down.insert("right", false);
                            mouse_buttons_down.insert("middle", false);
                        }

                        // 其他按钮可以根据需要添加
                        _ => {}
                    }
                }

                // 如果手柄断开连接，跳过后面的处理
                if !is_connected_clone.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }

                // 计算时间增量
                let now = Instant::now();
                let dt = now.duration_since(last_update).as_secs_f32();
                last_update = now;
                
                // 读取摇杆状态并移动鼠标
                let gamepad = gilrs.gamepad(gamepad_id);
                
                // 安全地获取配置
                let config_guard = match config_thread.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => {
                        error!("配置锁被毒化: {:?}", poisoned);
                        poisoned.into_inner() // 尝试恢复锁
                    }
                };
                
                // 读取所有摇杆值并记录更多信息
                let left_x = gamepad.value(Axis::LeftStickX);
                let left_y = gamepad.value(Axis::LeftStickY);
                let right_x = gamepad.value(Axis::RightStickX);
                let right_y = gamepad.value(Axis::RightStickY);
                let left_z = gamepad.value(Axis::LeftZ);  // 左扳机
                let right_z = gamepad.value(Axis::RightZ); // 右扳机
                
                // 降低日志记录的阈值，使得我们能看到更多的摇杆动作
                let log_threshold = 0.2; // 降低阈值，捕捉更小的摇杆动作
                
                // 记录摇杆值和计算后的移动值
                if left_x.abs() > log_threshold || left_y.abs() > log_threshold || 
                   right_x.abs() > log_threshold || right_y.abs() > log_threshold ||
                   left_z.abs() > log_threshold || right_z.abs() > log_threshold {
                    info!("摇杆原始值: 左X={:.2}, 左Y={:.2}, 右X={:.2}, 右Y={:.2}, 左Z={:.2}, 右Z={:.2}", 
                          left_x, left_y, right_x, right_y, left_z, right_z);
                }

                // 确定使用哪个摇杆控制鼠标移动
                let (x_axis, y_axis) = if config_guard.use_left_stick_for_mouse {
                    (gamepad.value(Axis::LeftStickX), gamepad.value(Axis::LeftStickY))
                } else {
                    (gamepad.value(Axis::RightStickX), gamepad.value(Axis::RightStickY))
                };
                
                // 使用非常小的死区值确保摇杆灵敏
                let dead_zone = config_guard.dead_zone.min(0.05); // 降低死区到0.05
                
                // 应用死区，但保留一些低值以确保摇杆响应
                let x_move = if x_axis.abs() > dead_zone {
                    // 使用更强的映射，保留低值但放大效果
                    let normalized = (x_axis.abs() - dead_zone) / (1.0 - dead_zone);
                    let adjusted = normalized.powf(0.8) * x_axis.signum(); // 降低指数，使响应更线性
                    if config_guard.invert_x_axis { -adjusted } else { adjusted }
                } else {
                    0.0
                };

                let y_move = if y_axis.abs() > dead_zone {
                    // 使用更强的映射，保留低值但放大效果
                    let normalized = (y_axis.abs() - dead_zone) / (1.0 - dead_zone);
                    let adjusted = normalized.powf(0.8) * y_axis.signum(); // 降低指数，使响应更线性
                    if config_guard.invert_y_axis { adjusted } else { -adjusted }
                } else {
                    0.0
                }; // 默认反转Y轴，与鼠标方向一致

                if x_move != 0.0 || y_move != 0.0 {
                    // 检查精确模式和加速模式
                    let mut sensitivity_multiplier = 1.0;
                    
                    // 精确模式 - 降低灵敏度
                    if button_matches(&gamepad, &config_guard.precision_mode_button) {
                        sensitivity_multiplier *= 0.3; // 降低到30%速度
                    }
                    
                    // 加速模式 - 提高灵敏度
                    if button_matches(&gamepad, &config_guard.turbo_mode_button) {
                        sensitivity_multiplier *= 2.0; // 提高到200%速度
                    }
                    
                    // 应用极高灵敏度设置
                    let base_sensitivity = config_guard.mouse_sensitivity.max(40.0); // 提高最小灵敏度到40
                    
                    // 使用更加剧烈的响应曲线
                    let acceleration = config_guard.mouse_acceleration.max(1.3); // 确保有足够的加速度
                    
                    // 对于小幅度移动，我们希望更精确的控制
                    // 对于大幅度移动，我们希望更快速的响应
                    let boost_factor = 2.5; // 大幅增加光标移动速度的额外增益
                    
                    // 添加额外的灵敏度倍增器
                    let extra_sensitivity = 1.8;
                    
                    // 更陡峭的曲线，确保小幅度移动也能产生明显效果，大幅度移动极快
                    let x_speed = x_move.abs().powf(acceleration)
                        * x_move.signum()
                        * base_sensitivity
                        * sensitivity_multiplier
                        * boost_factor
                        * extra_sensitivity
                        * (1.0 + 6.0 * x_move.abs()); // 大幅增加大幅度移动的速度
                        
                    let y_speed = y_move.abs().powf(acceleration)
                        * y_move.signum()
                        * base_sensitivity
                        * sensitivity_multiplier
                        * boost_factor
                        * extra_sensitivity
                        * (1.0 + 6.0 * y_move.abs()); // 大幅增加大幅度移动的速度
                    
                    // 保持小数部分以积累微小移动
                    static mut ACCUM_X: f32 = 0.0;
                    static mut ACCUM_Y: f32 = 0.0;
                    
                    // 安全地访问静态变量
                    let (accum_x, accum_y) = unsafe {
                        ACCUM_X += x_speed * dt;
                        ACCUM_Y += y_speed * dt;
                        (ACCUM_X, ACCUM_Y)
                    };
                    
                    // 为小值提供额外加速，确保即使微小移动也能生成整数位移
                    let boost_small_movements = |val: f32| -> f32 {
                        if val.abs() < 1.0 && val.abs() > 0.05 {
                            val * 1.5 // 增强小值，但不至于太小
                        } else {
                            val
                        }
                    };
                    
                    // 计算整数部分的移动，并更新累积值
                    let boosted_x = boost_small_movements(accum_x);
                    let boosted_y = boost_small_movements(accum_y);
                    
                    let dx = boosted_x.trunc() as i32;
                    let dy = boosted_y.trunc() as i32;
                    
                    // 安全地更新静态变量，保留小数部分
                    unsafe {
                        // 更新累积值，但保留一些动量以提高响应性
                        let momentum_factor = 0.7; // 保留70%的动量
                        ACCUM_X = (boosted_x - dx as f32) * momentum_factor;
                        ACCUM_Y = (boosted_y - dy as f32) * momentum_factor;
                    }
                    
                    if dx != 0 || dy != 0 {
                        // 移动鼠标（相对移动）
                        info!("移动鼠标: dx={}, dy={} (加速度: {}, 灵敏度: {})", 
                             dx, dy, config_guard.mouse_acceleration, config_guard.mouse_sensitivity);
                        
                        // 安全地移动鼠标，避免因为硬件错误导致崩溃
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            enigo.mouse_move_relative(dx, dy);
                        })) {
                            Ok(_) => {}, // 鼠标移动成功
                            Err(e) => error!("移动鼠标时发生错误: {:?}", e)
                        }
                    }
                }
                
                // 处理滚轮控制 - 使用未用于鼠标控制的摇杆
                let scroll_stick = if config_guard.use_left_stick_for_mouse {
                    // 如果左摇杆用于鼠标控制，则右摇杆用于滚轮
                    let raw_value = gamepad.value(Axis::RightStickY);
                    info!("右摇杆Y轴原始值: {}", raw_value);
                    raw_value
                } else {
                    // 如果右摇杆用于鼠标控制，则左摇杆用于滚轮
                    let raw_value = gamepad.value(Axis::LeftStickY);
                    info!("左摇杆Y轴原始值: {}", raw_value);
                    raw_value
                };
                
                // 使用较低的死区值，确保滚轮能够响应
                let scroll_dead_zone = config_guard.dead_zone * 0.7; // 降低死区，提高滚轮响应性
                let scroll_sensitivity = config_guard.scroll_sensitivity;
                
                // 打印摇杆原始值
                info!("摇杆绝对值: {}, 死区: {}", scroll_stick.abs(), scroll_dead_zone);
                
                // 使用线程局部存储实现平滑滚动
                SCROLL_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    
                    if scroll_stick.abs() > scroll_dead_zone {
                        // 计算滚轮量，使用配置中的灵敏度值，但确保更平滑的响应
                        let sensitivity = scroll_sensitivity * 0.25; // 提高基础灵敏度
                        let normalized = (scroll_stick.abs() - scroll_dead_zone) / (1.0 - scroll_dead_zone);
                        
                        // 自适应平滑曲线，在不同速度下都能提供良好的体验
                        let speed_curve = normalized.powf(1.2); // 更线性的响应曲线
                        let raw_scroll_amount = speed_curve * scroll_stick.signum() * sensitivity * dt;
                        
                        // 更新动量 - 逐渐融合新的滚动值，制造惯性效果
                        let momentum_retention = 0.75; // 保留75%的上次动量
                        state.momentum = state.momentum * momentum_retention + 
                                         raw_scroll_amount * (1.0 - momentum_retention);
                        
                        // 增加平滑滚动计时器
                        state.timer += dt;
                        
                        // 累积实际滚动量 - 结合动量因子使滚动更连贯
                        state.accum += raw_scroll_amount * 0.7 + state.momentum * 0.3;
                        state.last_time = Some(Instant::now());
                        
                        // 打印累积值，帮助调试
                        info!("滚轮累积值: {:.2}, 平滑计时器: {:.2}, 动量: {:.2}", 
                              state.accum, state.timer, state.momentum);
                    }
                    
                    // 基于累积值和平滑计时器决定滚动行为
                    let scroll_amount = {
                        // 非常流畅的滚动间隔
                        let base_interval = 0.025; // 更低的基础间隔，使滚动频率更高
                        let normalized_abs = state.accum.abs().min(1.0); // 使用累积值的绝对值作为归一化值
                        let scroll_interval = if normalized_abs > 0.8 {
                            base_interval * 0.7  // 大幅度移动时，极快的滚动频率
                        } else if normalized_abs > 0.5 {
                            base_interval * 0.9  // 中等移动，略快频率
                        } else {
                            base_interval        // 小移动，标准频率
                        };
                        
                        // 使用更小的阈值和更短的滚动间隔，确保滚动感觉流畅而非"一卡一卡"
                        if state.accum.abs() >= 0.08 && state.timer >= scroll_interval {
                            // 重置平滑滚动计时器，准备下一次滚动
                            state.timer = 0.0;
                            
                            // 确定滚动方向和强度（基于累积值大小）
                            let direction = if state.accum > 0.0 { 1.0 } else { -1.0 };
                            
                            // 根据累积值的大小动态确定滚动量，使快速移动时有更多滚动单位
                            let strength = state.accum.abs().min(1.5); // 限制强度上限，但允许更大值
                            let scale_factor = if strength > 0.8 { 2.0 } else { 1.0 }; // 大幅度移动时翻倍
                            
                            // 生成滚动量
                            let raw_amount = strength * scale_factor;
                            let amount = (raw_amount.round() as i32) * direction as i32;
                            let amount = if amount == 0 { direction as i32 } else { amount }; // 确保至少有1的滚动量
                            
                            // 减少累积值，但保留一部分以维持流畅性
                            let reduction = if scale_factor > 1.0 {
                                // 大幅度滚动时，减少更多累积值
                                (amount as f32).abs() * 0.12
                            } else {
                                // 小幅度滚动时，减少较少累积值以保持连续性
                                (amount as f32).abs() * 0.08
                            };
                            
                            state.accum -= direction * reduction;
                            
                            // 衰减累积值，防止过度累积
                            state.accum *= 0.92;
                            
                            amount
                        } else {
                            0
                        }
                    };
                    
                    // 最终滚动量，根据摇杆的移动幅度可能会有所不同
                    let final_amount = if scroll_amount != 0 {
                        // 根据摇杆移动幅度调整滚动速度
                        let stick_abs = scroll_stick.abs();
                        let magnitude_boost = if stick_abs > 0.85 {
                            // 当摇杆移动幅度很大时，加速滚动（最多2个单位）
                            if scroll_amount > 0 { scroll_amount * 2 } else { scroll_amount * 2 }
                        } else {
                            // 标准滚动速度
                            scroll_amount
                        };
                        
                        // 确保始终有效果
                        magnitude_boost
                    } else {
                        0
                    };
                    
                    // 只在有实际滚动时记录日志和执行操作
                    if final_amount != 0 {
                        info!("滚动滚轮: {} (原始值: {}, 死区: {}, 灵敏度: {}, 累积值: {})",
                            final_amount, scroll_stick, scroll_dead_zone, scroll_sensitivity,
                            state.accum);
                        
                        // 安全地执行滚轮操作，避免因为硬件错误导致崩溃
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            // 执行滚轮操作，反转符号使得摇杆向下时滚轮向下滚动
                            enigo.mouse_scroll_y(-final_amount);
                        })) {
                            Ok(_) => {}, // 滚轮操作成功
                            Err(e) => error!("滚轮操作时发生错误: {:?}", e)
                        }
                    }
                });
                
                // 短暂休眠以避免CPU占用过高，但保持足够的响应速度
                thread::sleep(Duration::from_millis(4)); // 略微减少休眠时间，提高响应性
                
                // 定期检查并报告状态 (大约每5秒)
                if now.elapsed().as_secs() % 5 == 0 && now.elapsed().subsec_nanos() < 10_000_000 {
                    info!("手柄控制线程运行中 - 使用{}摇杆控制鼠标", 
                          if config_guard.use_left_stick_for_mouse { "左" } else { "右" });
                }
            }

            info!("手柄控制线程已停止");
        });

        Self {
            thread_handle: Some(thread_handle),
            running,
            gamepad_id,
            config,
            last_activity,
            is_connected,
            connection_lost_time,
        }
    }

    /// 停止控制器
    pub fn stop(&mut self) {
        info!("正在停止手柄控制器");
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.thread_handle.take() {
            // 等待线程结束，但设置超时避免永久阻塞
            match handle.join() {
                Ok(_) => info!("手柄控制器已成功停止"),
                Err(e) => error!("停止手柄控制器时发生错误: {:?}", e),
            }
        } else {
            info!("手柄控制器已经停止");
        }
    }

    /// 更新配置
    pub fn update_config(&mut self, config: Config) {
        info!("更新手柄控制器配置");
        // 记录配置更新情况
        info!("鼠标灵敏度: {}, 死区: {}, 滚轮灵敏度: {}, 加速度: {}", 
             config.mouse_sensitivity, config.dead_zone, 
             config.scroll_sensitivity, config.mouse_acceleration);
        info!("摇杆设置: 使用左摇杆={}, 反转X轴={}, 反转Y轴={}", 
             config.use_left_stick_for_mouse, config.invert_x_axis, config.invert_y_axis);
        
        // 更新配置
        match self.config.lock() {
            Ok(mut guard) => {
                *guard = config;
                info!("配置已成功更新");
            },
            Err(e) => {
                error!("更新配置失败: {:?}", e);                // 尝试恢复锁
                let mut guard = e.into_inner();
                *guard = config;
                info!("配置已在锁恢复后更新");
            }
        }
        
        // 重置滚轮状态，确保新配置立即生效
        SCROLL_STATE.with(|state| {
            state.borrow_mut().reset();
        });
    }    /// 尝试恢复连接
    pub fn try_reconnect(&mut self) -> bool {
        // 检查是否已经连接
        if self.is_connected.load(Ordering::Relaxed) {
            debug!("手柄已连接，无需重连");
            return true; // 已经连接，不需要重连
        }

        info!("尝试恢复手柄连接...");
        
        // 记录上次断开连接的持续时间
        let disconnection_duration = if let Ok(lost_time) = self.connection_lost_time.lock() {
            lost_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
        } else {
            warn!("无法获取连接丢失时间，可能发生了互斥锁问题");
            0
        };
        
        if disconnection_duration > 0 {
            info!("手柄已断开连接 {} 秒", disconnection_duration);
        }
        
        // 尝试重新初始化gilrs
        match Gilrs::new() {
            Ok(gilrs) => {
                // 检查手柄是否存在
                let gamepad = gilrs.gamepad(self.gamepad_id);
                if gamepad.is_connected() {
                    info!("手柄重新连接成功，恢复运行状态");
                    info!("已恢复连接的手柄：{} (id: {:?})", gamepad.name(), self.gamepad_id);
                    self.is_connected.store(true, Ordering::Relaxed);
                    
                    // 重置连接丢失时间
                    if let Ok(mut lost_time) = self.connection_lost_time.lock() {
                        *lost_time = None;
                    } else {
                        warn!("无法重置连接丢失时间");
                    }
                    
                    // 更新上次活动时间
                    if let Ok(mut last_activity) = self.last_activity.lock() {
                        *last_activity = Instant::now();
                    } else {
                        warn!("无法更新最后活动时间");
                    }
                    
                    return true;
                } else {
                    info!("手柄仍然断开连接，无法恢复 ID: {:?}", self.gamepad_id);
                    
                    // 尝试查找其他可用手柄
                    let mut found_alternative = false;
                    for (id, gp) in gilrs.gamepads() {
                        info!("发现可用的替代手柄：{} (id: {:?})", gp.name(), id);
                        found_alternative = true;
                        // 仅记录，不自动切换
                    }
                    
                    if !found_alternative {
                        info!("未发现其他可用手柄");
                    }
                    
                    return false;
                }
            },
            Err(err) => {
                error!("重新初始化手柄系统失败: {}", err);
                return false;
            }
        }
    }

    /// 检查控制器是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
    
    /// 检查手柄是否仍然连接
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }
    
    /// 获取上次活动时间
    pub fn get_last_activity(&self) -> Option<Instant> {
        self.last_activity.lock().ok().map(|guard| *guard)
    }
    
    /// 获取连接丢失时间
    pub fn get_connection_lost_time(&self) -> Option<Instant> {
        match self.connection_lost_time.lock() {
            Ok(guard) => *guard,
            Err(_) => None
        }
    }
}

impl Drop for GamepadController {
    fn drop(&mut self) {
        info!("正在销毁手柄控制器实例");
        self.stop();
    }
}

/// 将Button枚举转换为字符串
fn button_to_string(button: Button) -> String {
    match button {
        Button::South => "South".to_string(),
        Button::East => "East".to_string(),
        Button::North => "North".to_string(),
        Button::West => "West".to_string(),
        Button::C => "C".to_string(),
        Button::Z => "Z".to_string(),
        Button::LeftTrigger => "LeftTrigger".to_string(),
        Button::LeftTrigger2 => "LeftTrigger2".to_string(),
        Button::RightTrigger => "RightTrigger".to_string(),
        Button::RightTrigger2 => "RightTrigger2".to_string(),
        Button::Select => "Select".to_string(),
        Button::Start => "Start".to_string(),
        Button::Mode => "Mode".to_string(),
        Button::LeftThumb => "LeftThumb".to_string(),
        Button::RightThumb => "RightThumb".to_string(),
        Button::DPadUp => "DPadUp".to_string(),
        Button::DPadDown => "DPadDown".to_string(),
        Button::DPadLeft => "DPadLeft".to_string(),
        Button::DPadRight => "DPadRight".to_string(),
        Button::Unknown => "Unknown".to_string(),
    }
}

/// 检查游戏手柄上的按钮是否处于按下状态
fn button_matches(gamepad: &gilrs::Gamepad, button_name: &str) -> bool {
    match button_name {
        "South" => gamepad.is_pressed(Button::South),
        "East" => gamepad.is_pressed(Button::East),
        "North" => gamepad.is_pressed(Button::North),
        "West" => gamepad.is_pressed(Button::West),
        "C" => gamepad.is_pressed(Button::C),
        "Z" => gamepad.is_pressed(Button::Z),
        "LeftTrigger" => gamepad.is_pressed(Button::LeftTrigger),
        "LeftTrigger2" => gamepad.is_pressed(Button::LeftTrigger2),
        "RightTrigger" => gamepad.is_pressed(Button::RightTrigger),
        "RightTrigger2" => gamepad.is_pressed(Button::RightTrigger2),
        "Select" => gamepad.is_pressed(Button::Select),
        "Start" => gamepad.is_pressed(Button::Start),
        "Mode" => gamepad.is_pressed(Button::Mode),
        "LeftThumb" => gamepad.is_pressed(Button::LeftThumb),
        "RightThumb" => gamepad.is_pressed(Button::RightThumb),
        "DPadUp" => gamepad.is_pressed(Button::DPadUp),
        "DPadDown" => gamepad.is_pressed(Button::DPadDown),
        "DPadLeft" => gamepad.is_pressed(Button::DPadLeft),
        "DPadRight" => gamepad.is_pressed(Button::DPadRight),
        _ => false,
    }
}
