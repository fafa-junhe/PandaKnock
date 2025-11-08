mod config;
mod knock;

use async_std::task::sleep;
use iced::event::{self, Event};
use iced::window;
use knock::shoot;

use iced::widget::{button, column, text_input};
use iced::{Alignment, Element, Font, Subscription, Task};
use iced_toasts::{toast, toast_container, ToastContainer, ToastId, ToastLevel};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnockType {
    Open,
    Close,
}

#[derive(Debug)]
struct PandaKnocking<'a, Message> {
    host: String,
    ports_str: String,
    ports: Vec<u16>,
    close_ports_str: String,
    close_ports: Vec<u16>,
    toasts: ToastContainer<'a, Message>,
    delay: u64,
    is_knocking: bool,        // 新增状态：跟踪敲门 过程是否正在进行
    is_close_requested: bool, // 是否用户点击了关闭按钮
}

#[derive(Debug, Clone)]
enum Message {
    HostInputChanged(String),
    PortInputChanged(String),
    DelayInputChanged(String),
    ClosePortInputChanged(String),
    DismissToast(ToastId),
    PushToast((String, ToastLevel)),
    // 新增消息：用于处理每一步的敲门操作
    // usize 参数是下一个要敲门的端口的索引
    KnockStep((KnockType, usize)),
    SaveButtonPressed,
    KnockPressed,
    CloseKnockPressed,
    EventOccurred(Event),
    ExitApp,
    SaveCompleted(Result<(), String>), // 用于接收保存任务的结果
}

impl Default for PandaKnocking<'_, Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl PandaKnocking<'_, Message> {
    fn new() -> Self {
        // 从文件加载配置，如果失败则使用默认值
        let config = config::load_or_create();

        let mut app = Self {
            toasts: toast_container(Message::DismissToast),
            host: config.host,
            ports_str: config.ports_str,
            ports: Vec::new(), // 将在下面解析
            close_ports: Vec::new(),
            close_ports_str: config.close_ports_str,
            delay: config.delay,
            is_knocking: false,
            is_close_requested: false,
        };

        // 确保 `ports` 向量与加载的 `ports_str` 同步
        app.parse_ports();
        app.parse_close_ports();
        app
    }

    fn parse_ports(&mut self) {
        self.ports = self.parse_port_str(&self.ports_str.clone(), "开启");
    }

    // 辅助函数：解析关闭端口
    fn parse_close_ports(&mut self) {
        self.close_ports = self.parse_port_str(&self.close_ports_str.clone(), "关闭");
    }

    // 通用端口解析逻辑
    fn parse_port_str(&mut self, port_str: &str, kind: &str) -> Vec<u16> {
        let mut parsed_ports = Vec::new();
        for (i, port) in port_str.split(',').enumerate() {
            let trimmed = port.trim();
            if trimmed.is_empty() {
                continue;
            } // 忽略空字符串
            match trimmed.parse::<u16>() {
                Ok(num) => parsed_ports.push(num),
                Err(err) => {
                    let msg = format!(
                        "第 {} 个{}端口解析失败：'{}' ({})",
                        i + 1,
                        kind,
                        trimmed,
                        err
                    );
                    self.toasts.push(toast(&msg).level(ToastLevel::Error));
                }
            }
        }
        parsed_ports
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::HostInputChanged(new_value) => {
                self.host = new_value;
                Task::none()
            }
            Message::PortInputChanged(new_port) => {
                self.ports_str = new_port;
                self.parse_ports();
                Task::none()
            }
            Message::ClosePortInputChanged(value) => {
                self.close_ports_str = value;
                self.parse_close_ports();
                Task::none()
            }
            Message::PushToast(tup) => {
                let (message, level) = tup;
                println!("{}", message);
                self.toasts.push(toast(&message).level(level));
                Task::none()
            }
            Message::DelayInputChanged(new_delay) => match new_delay.trim().parse::<u64>() {
                Ok(num) => {
                    self.delay = num;
                    Task::none()
                }
                Err(err) => Task::perform(
                    async move {
                        (
                            format!("延迟解析失败：'{}' ({})", new_delay.trim(), err),
                            ToastLevel::Error,
                        )
                    },
                    Message::PushToast,
                ),
            },
            // --- 核心逻辑改动在这里 ---
            Message::KnockPressed => {
                // 如果正在敲门，或者没有端口，则不执行任何操作
                if self.is_knocking || self.ports.is_empty() {
                    return Task::none();
                }

                // 设置状态为正在敲门，这将禁用按钮
                self.is_knocking = true;

                // 直接发送第一个步骤的消息来启动任务链
                Task::perform(async move { (KnockType::Open, 0) }, Message::KnockStep)
            }
            Message::CloseKnockPressed => {
                if self.is_knocking || self.close_ports.is_empty() {
                    return Task::none();
                }
                self.is_knocking = true;
                // --- 修改：启动“关闭”序列的第一步 ---
                Task::perform(async move { (KnockType::Close, 0) }, Message::KnockStep)
            }
            Message::KnockStep(data) => {
                let (knock_type, index) = data;
                let (ports_to_use, kind_str) = match knock_type {
                    KnockType::Open => (&self.ports, "开启"),
                    KnockType::Close => (&self.close_ports, "关闭"),
                };

                if index > 0 {
                    let prev_index = index - 1;
                    if let Some(&port) = ports_to_use.get(prev_index) {
                        let msg = format!(
                            "第 {} 个{}端口成功发送敲门指令：'{}:{}'",
                            prev_index + 1,
                            kind_str,
                            self.host,
                            port
                        );
                        self.toasts.push(toast(&msg).level(ToastLevel::Success));
                    }
                }

                if let Some(&port) = ports_to_use.get(index) {
                    let host_clone = self.host.clone();
                    let delay = self.delay;
                    Task::perform(
                        async move {
                            let addr = format!("{}:{}", host_clone, port);
                            shoot(addr).await;
                            sleep(Duration::from_millis(delay)).await;
                            (knock_type, index + 1)
                        },
                        |(ty, idx)| Message::KnockStep((ty, idx)),
                    )
                } else {
                    self.is_knocking = false;
                    let msg = format!("所有{}端口敲门完成!", kind_str);
                    self.toasts.push(toast(&msg).level(ToastLevel::Info));

                    // 如果是“关闭”序列结束 → 自动退出程序
                    if let KnockType::Close = knock_type {
                        if self.is_close_requested {
                            return Task::perform(async {}, |_| Message::ExitApp);
                        }
                    }

                    Task::none()
                }
            }
            Message::DismissToast(id) => {
                self.toasts.dismiss(id);
                Task::none()
            }
            Message::SaveButtonPressed => {
                // 从当前状态创建一个 config 对象
                let config_to_save = config::Config {
                    host: self.host.clone(),
                    ports_str: self.ports_str.clone(),
                    close_ports_str: self.close_ports_str.clone(),
                    delay: self.delay,
                };

                // 创建一个异步任务来保存配置
                // Task::perform 会在后台线程池中运行这个 future，不会阻塞 UI
                Task::perform(
                    async move { config::save(&config_to_save) },
                    Message::SaveCompleted,
                )
            }
            Message::SaveCompleted(result) => {
                match result {
                    Ok(_) => {
                        self.toasts
                            .push(toast("配置保存成功!").level(ToastLevel::Success));
                    }
                    Err(e) => {
                        self.toasts.push(
                            toast(format!("配置保存失败: {}", e).as_str()).level(ToastLevel::Error),
                        );
                    }
                }
                Task::none()
            }
            Message::EventOccurred(event) => {
                if let Event::Window(window::Event::CloseRequested) = event {
                    // 如果当前正在敲门或已经在关闭，则忽略
                    if self.is_knocking {
                        return Task::none();
                    }
                    self.toasts.push(
                        toast("检测到关闭请求，正在发送关闭端口序列...").level(ToastLevel::Info),
                    );

                    // 标记为“请求关闭”
                    self.is_close_requested = true;

                    // 如果没有关闭序列，则直接退出
                    if self.close_ports.is_empty() {
                        return Task::perform(async {}, |_| Message::ExitApp);
                    }

                    // 启动关闭敲门序列
                    self.is_knocking = true;
                    return Task::perform(async move { (KnockType::Close, 0) }, Message::KnockStep);
                };
                Task::none()
            }
            Message::ExitApp => {
                std::process::exit(0);
            }
        }
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        let host_input = text_input("请输入ip...", &self.host.as_str())
            .on_input(Message::HostInputChanged)
            .padding(10)
            .size(20);

        let ports_input = text_input("请输入敲门的端口，用逗号分隔...", &self.ports_str.as_str())
            .on_input(Message::PortInputChanged)
            .padding(10)
            .size(20);

        let close_ports_input = text_input("关闭端口序列 (逗号分隔)...", &self.close_ports_str)
            .on_input(Message::ClosePortInputChanged)
            .padding(10)
            .size(20);

        let delay_inputs = text_input("端口敲门间隔延迟...", &self.delay.to_string())
            .on_input(Message::DelayInputChanged)
            .padding(10)
            .size(20);

        let form = column![host_input, ports_input, close_ports_input, delay_inputs]
            .padding(30)
            .spacing(20)
            .align_x(Alignment::Center);

        let mut knock_button = button("端口敲门").padding(20);
        if !self.is_knocking {
            knock_button = knock_button.on_press(Message::KnockPressed);
        }
        let mut close_knock_button = button("关闭端口").padding(20);
        if !self.is_knocking {
            close_knock_button = close_knock_button.on_press(Message::CloseKnockPressed);
        }
        // --- 新增保存按钮 ---
        let save_button = button("保存配置")
            .on_press(Message::SaveButtonPressed)
            .padding(20);

        // 将两个按钮放在一行
        let buttons = iced::widget::row![knock_button, close_knock_button, save_button]
            .spacing(10)
            .align_y(Alignment::Center);

        let interface = column![form, buttons]
            .align_x(Alignment::Center)
            .spacing(20);
        self.toasts.view(interface)
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen().map(Message::EventOccurred)
    }
}

fn main() -> iced::Result {
    #[cfg(target_os = "windows")]
    let default_font = Font::with_name("Microsoft YaHei UI");

    #[cfg(target_os = "macos")]
    let default_font = Font::with_name("PingFang SC");

    #[cfg(target_os = "linux")]
    let default_font = Font::with_name("Noto Sans CJK SC");

    iced::application("熊猫端口敲门器", PandaKnocking::update, PandaKnocking::view)
        .window_size(iced::Size::new(400.0, 500.0))
        .default_font(default_font)
        .exit_on_close_request(false)
        .subscription(PandaKnocking::subscription)
        .run()
}
