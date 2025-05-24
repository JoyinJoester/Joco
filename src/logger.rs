// 增强型日志模块，提供更全面的日志功能
use log::{LevelFilter, Record, Level, Metadata};
use simple_logger::SimpleLogger;
use std::fs::{File, OpenOptions, create_dir_all};
use std::path::{Path, PathBuf};
use std::io::{Write, Error};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::Write as FmtWrite;
use std::sync::Arc;
use dirs;
use chrono;

/// 增强型日志器，支持文件轮转和自定义格式化
pub struct EnhancedLogger {
    log_file: Arc<Mutex<Option<File>>>,
    log_level: LevelFilter,
    max_file_size: u64,  // 最大日志文件大小（字节）
    log_directory: PathBuf,
    current_log_path: PathBuf,
}

impl EnhancedLogger {
    /// 创建新的增强型日志器
    pub fn new(log_level: LevelFilter) -> Result<Self, Error> {
        // 确定日志目录
        let log_directory = if let Some(app_dir) = dirs::data_local_dir() {
            let mut dir = app_dir;
            dir.push("GamepadMouseControl");
            dir.push("logs");
            dir
        } else {
            PathBuf::from("./logs")
        };
        
        // 确保日志目录存在
        create_dir_all(&log_directory)?;
        
        // 确定初始日志文件路径
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let log_filename = format!("gamepad-mouse-control-{}.log", timestamp);
        let log_path = log_directory.join(&log_filename);
        
        // 创建日志文件
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&log_path)?;
        
        Ok(Self {
            log_file: Arc::new(Mutex::new(Some(file))),
            log_level,
            max_file_size: 5 * 1024 * 1024, // 默认为5MB
            log_directory,
            current_log_path: log_path,
        })
    }
    
    /// 设置最大日志文件大小
    pub fn with_max_file_size(mut self, size_in_bytes: u64) -> Self {
        self.max_file_size = size_in_bytes;
        self
    }
    
    /// 初始化日志系统
    pub fn init(self) -> Result<(), log::SetLoggerError> {
        // 使用 simple_logger 处理控制台输出
        SimpleLogger::new()
            .with_level(self.log_level)
            .init()?;
        
        // 注册我们自己的日志处理器来处理文件输出
        log::set_max_level(self.log_level);
        
        Ok(())
    }
    
    /// 检查日志文件大小并在必要时进行轮转
    fn rotate_log_if_needed(&self) -> Result<(), Error> {
        let file_lock = self.log_file.lock().unwrap();
        
        if let Some(file) = &*file_lock {
            // 检查当前文件大小
            let metadata = file.metadata()?;
            if metadata.len() > self.max_file_size {
                // 需要轮转日志文件
                // 在这里实现轮转逻辑
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let new_filename = format!("gamepad-mouse-control-{}.log", timestamp);
                let new_path = self.log_directory.join(&new_filename);
                
                // 创建新的日志文件
                let new_file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&new_path)?;
                
                // 更新文件引用
                drop(file_lock);
                let mut file_lock = self.log_file.lock().unwrap();
                *file_lock = Some(new_file);
            }
        }
        
        Ok(())
    }
    
    /// 写入日志记录到文件
    fn write_log(&self, record: &Record) -> Result<(), Error> {
        // 格式化日志消息
        let mut message = String::new();
        let level_str = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };
        
        // 格式化时间戳
        let now = chrono::Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
        
        // 构建日志消息
        write!(
            &mut message,
            "[{}] {} [{}:{}] {}\n",
            timestamp,
            level_str,
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.args()
        ).ok();
        
        // 检查是否需要轮转日志
        self.rotate_log_if_needed()?;
        
        // 写入日志文件
        let mut file_lock = self.log_file.lock().unwrap();
        if let Some(file) = &mut *file_lock {
            file.write_all(message.as_bytes())?;
            file.flush()?;
        }
        
        Ok(())
    }
}

/// 自定义日志Handler
pub struct FileLogger {
    logger: Arc<EnhancedLogger>,
}

impl FileLogger {
    pub fn new(logger: EnhancedLogger) -> Self {
        Self {
            logger: Arc::new(logger),
        }
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.logger.log_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Err(e) = self.logger.write_log(record) {
                eprintln!("日志写入失败: {}", e);
            }
        }
    }

    fn flush(&self) {
        // 日志在每次写入后都会刷新，此处无需额外操作
    }
}

/// 实现增强日志初始化功能
pub fn initialize_enhanced_logging(level: Option<LevelFilter>) -> Result<(), String> {
    let log_level = level.unwrap_or(LevelFilter::Info);
    
    // 创建增强型日志器
    match EnhancedLogger::new(log_level) {
        Ok(logger) => {
            match logger.init() {
                Ok(_) => {
                    log::info!("增强型日志系统已初始化（级别：{:?}）", log_level);
                    Ok(())
                },
                Err(e) => Err(format!("无法初始化日志系统: {}", e))
            }
        },
        Err(e) => Err(format!("无法创建日志文件: {}", e))
    }
}

/// 创建一个简单的日志初始化函数，备用
pub fn initialize_simple_logging(level: Option<LevelFilter>) -> Result<(), String> {
    let log_level = level.unwrap_or(LevelFilter::Info);
    
    if let Err(e) = SimpleLogger::new().with_level(log_level).init() {
        return Err(format!("无法初始化日志系统: {}", e));
    }
    
    log::info!("简单日志系统已初始化（级别：{:?}）", log_level);
    Ok(())
}