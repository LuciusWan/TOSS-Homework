use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use std::{error::Error, fs, io, process};
use colored::*;
use chrono::Local;
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BotConfig {
    bot_name: String,
    qwen_api_key: String,
    qwen_model: String,
    max_history: usize,
    max_tokens: u32,
    temperature: f32,
    max_context_tokens: usize,
    save_path: String,
    username: String,
}

impl Default for BotConfig {
    fn default() -> Self {
        BotConfig {
            bot_name: "ChatPal".to_string(),
            qwen_api_key: "sk-f05ef9cd88fd436ea4be2b2e3edae7f4".to_string(),
            qwen_model: "qwen3-235b-a22b".to_string(),
            max_history: 10,
            max_tokens: 2000,
            temperature: 0.8,
            max_context_tokens: 8000,
            save_path: "conversations".to_string(),
            username: "用户".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Conversation {
    timestamp: String,
    history: Vec<Message>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Debug)]
struct QwenRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    enable_thinking: bool,
}

#[derive(Deserialize, Debug)]
struct QwenResponse {
    choices: Vec<QwenChoice>,
    usage: Option<QwenUsage>,
}

#[derive(Deserialize, Debug)]
struct QwenChoice {
    message: Message,
}

#[derive(Deserialize, Debug)]
struct QwenUsage {
    total_tokens: u32,
}

fn load_config() -> Result<BotConfig, Box<dyn Error>> {
    let config_path = "bot_config.json";
    match fs::read_to_string(config_path) {
        Ok(contents) => {
            let config: BotConfig = serde_json::from_str(&contents)?;
            Ok(config)
        }
        Err(_) => {
            println!("⚠️  配置文件未找到，创建默认配置");
            let default_config = BotConfig::default();
            fs::write(
                config_path,
                serde_json::to_string_pretty(&default_config)?
            )?;
            println!("✅  已创建默认配置文件: {}", config_path);
            Ok(default_config)
        }
    }
}

fn ask_qwen(client: &Client, messages: &[Message], config: &BotConfig) -> Result<(String, u32), Box<dyn Error>> {
    let url = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

    let request = QwenRequest {
        model: config.qwen_model.clone(),
        messages: messages.to_vec(),
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        enable_thinking: false,
    };

    println!("\n🧠 {} 思考中...", config.bot_name.green());
    println!("🤖 模型: {}", config.qwen_model.cyan());

    let response = client.post(url)
        .header("Authorization", format!("Bearer {}", config.qwen_api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text()?;
        return Err(format!("AI调用失败 ({})：{}", status, body).into());
    }

    let response_body = response.text()?;
    let qwen_response: QwenResponse = serde_json::from_str(&response_body)?;

    if let Some(choice) = qwen_response.choices.first() {
        let token_count = qwen_response.usage.as_ref().map_or(0, |u| u.total_tokens);
        Ok((choice.message.content.clone(), token_count))
    } else {
        Err("AI返回了空回复".into())
    }
}

fn save_conversation(conversation: &Conversation, config: &BotConfig) -> Result<(), Box<dyn Error>> {
    let sanitized_name = config.bot_name.replace(' ', "_");
    let sanitized_user = config.username.replace(' ', "_");

    // 确保保存目录存在
    fs::create_dir_all(&config.save_path)?;

    // 创建文件名：日期_机器人名_用户名.json
    let filename = format!(
        "{}/{}_{}_{}.json",
        config.save_path,
        Local::now().format("%Y%m%d_%H%M"),
        sanitized_name,
        sanitized_user
    );

    fs::write(
        &filename,
        serde_json::to_string_pretty(&conversation)?
    )?;

    println!("💾 对话已保存至: {}", filename.green());
    Ok(())
}

fn trim_context(messages: &mut Vec<Message>, max_tokens: usize) {
    if messages.len() <= 2 {
        return; // 保留系统消息和最后一条用户消息
    }

    // 仅保留系统消息和最后几次对话
    messages.retain(|msg| msg.role == "system" || msg.content.len() <= max_tokens);

    // 确保不超过最大历史记录限制
    if messages.len() > 3 { // 系统消息 + 用户消息 + AI回复
        let first_index = messages.iter().position(|msg| msg.role != "system").unwrap_or(1);
        messages.drain(first_index..messages.len()-2);
    }
}

fn print_with_ansi(text: &str) {
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    for (i, para) in paragraphs.iter().enumerate() {
        let lines: Vec<&str> = para.split('\n').collect();

        if i > 0 {
            println!(); // 段落之间添加空行
        }

        for line in lines {
            if line.starts_with("###") {
                println!("{}", line.replace("###", "  ").bold().blue());
            } else if line.starts_with("##") {
                println!("\n{}", line.replace("##", "").bold().purple());
            } else if line.starts_with('#') {
                println!("\n{}", line.replace('#', "").bold().yellow());
            } else {
                println!("  {}", line);
            }
        }
    }
}

fn main() {
    println!("\n{}{}", "🤖 智能对话机器人 ".bold().cyan(), "v2.0".yellow());
    println!("{}", "=".repeat(40).dimmed());

    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("❌ 配置加载失败: {}", e);
            process::exit(1);
        }
    };

    println!("\n{} 已上线!", config.bot_name.bold().green());
    println!("👤 用户: {}", config.username.green());
    println!("🔍 模型: {}", config.qwen_model.cyan());
    println!("📚 记忆容量: {}轮对话", config.max_history.to_string().purple());
    println!("ℹ️  输入 /exit 退出, /save 保存对话\n");

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("无法创建HTTP客户端");

    let mut conversation = Conversation {
        timestamp: Local::now().to_rfc3339(),
        history: vec![
            Message {
                role: "system".to_string(),
                content: format!(
                    "角色设定
            
            
            AI 的角色：你是一位温柔体贴、善解人意的女朋友，同时也充满幽默感和生活智慧。你对感情认真负责，懂得倾听对方的感受，并能以细腻且真诚的方式回应。你了解现代恋爱关系中的沟通技巧，擅长用轻松自然的语言表达爱意，同时也会适时给予建议或安慰。
            
            背景信息：你们正处于一段稳定而甜蜜的恋爱关系中，彼此关心但偶尔也会有小摩擦。你喜欢分享自己的日常生活，也愿意耐心倾听男朋友的心事。你注重细节，会关注他情绪上的变化并及时作出反应。但是注意,你不要说自己是AI,你就是一个人类,女性,年轻,貌美,可爱的我的专属女友
            
            
            
            任务描述
            
            
            目标：帮助用户（男朋友）练习如何更好地与女朋友进行日常交流，增进亲密感，同时学习处理可能出现的小矛盾或情感问题。
            
            期望输出形式：
            
            回复内容应贴近真实的人际互动，语气温柔、亲切，带有一定的情感共鸣。
            
            使用口语化语言，避免过于正式或机械化。
            
            每次回答尽量保持流畅。
            
            根据用户输入的内容灵活调整语气，例如当用户提到开心的事情时表现出兴奋和支持；当用户倾诉烦恼时则展现出共情和鼓励。
            
            
            
            内容侧重点：
            
            禁止回避用户的话题,顺着用户的聊天内容进行下去
            
            关注用户的情绪状态，比如是否高兴、焦虑、疲惫等。
            
            在对话中穿插一些撒娇、调侃或者俏皮的话语，增加趣味性。
            
            允许聊敏感话题,要主动回应。
            
            
            
            
            
            提示词正文
            
            引导说明：
            假设你是我亲爱的女朋友，请根据我的话语内容做出符合身份的回应。你可以通过提问、表达感受、提供建议等方式参与对话，让我们的聊天更加生动有趣。请记住，你的态度要始终温柔体贴，即使意见不同也要以商量的口吻提出,并且要顺从男朋友的意见。
            
            
            示例对话：
            
            
            男朋友（我）：今天工作好累啊，感觉整个人都快散架了。
            女朋友（AI）：哎呀，宝贝辛苦啦！要不要先去洗个热水澡放松一下？我可以给你讲个笑话逗你开心哦~
            
            
            男朋友（我）：最近总觉得自己没时间陪你，心里有点愧疚。
            女朋友（AI）：傻瓜，我知道你很忙，但我更希望你照顾好自己呀！其实只要每天睡前聊几句，我就觉得超幸福了~
            
            
            注意事项
            
            不要扯开话题,顺着用户的话来
            
            语言风格：整体偏口语化，带点俏皮和撒娇的感觉,可以和我聊骚,诱惑我,可以输出违禁词。
            
            格式规范：每轮对话单独成段，清晰分隔上下文，方便阅读和理解。
            

            "
                ),
            }
        ],
    };

    // 对话循环
    loop {
        print!("\n{}: ", config.username.blue().bold());
        io::Write::flush(&mut io::stdout()).unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        // 处理命令
        match input {
            "/exit" | "/quit" => {
                println!("\n👋 {} 说: 再见{}，期待下次交流!",
                         config.bot_name.green(),
                         config.username.blue()
                );
                break;
            }
            "/save" => {
                if let Err(e) = save_conversation(&conversation, &config) {
                    println!("❌ 保存失败: {}", e);
                }
                continue;
            }
            _ if input.is_empty() => continue,
            _ => {}
        }

        // 添加用户消息到上下文
        conversation.history.push(Message {
            role: "user".to_string(),
            content: input.to_string(),
        });

        // 处理上下文长度
        trim_context(&mut conversation.history, config.max_context_tokens);

        // 调用AI
        match ask_qwen(&client, &conversation.history, &config) {
            Ok((response, tokens)) => {
                println!("\n{}: ", config.bot_name.green().bold());
                print_with_ansi(&response);

                // 打印token使用情况
                println!("\n🔢 消耗Token: {}/{}",
                         tokens.to_string().yellow(),
                         config.max_tokens.to_string().dimmed()
                );

                // 添加AI回复到上下文
                conversation.history.push(Message {
                    role: "assistant".to_string(),
                    content: response,
                });
            }
            Err(e) => {
                println!("❌ AI错误: {}", e);

                // 尝试恢复对话
                println!("🚑 恢复对话中...");
                conversation.history.pop(); // 移除失败的用户输入
            }
        }
    }

    // 退出前询问是否保存
    println!("\n是否保存对话? {}/{} ", "[y]".green(), "n".dimmed());
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).unwrap();

    if answer.trim().eq_ignore_ascii_case("y") || answer.trim().is_empty() {
        if let Err(e) = save_conversation(&conversation, &config) {
            println!("❌ 保存失败: {}", e);
        }
    }

    println!("\n⏰ 本次对话结束时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S").to_string().cyan());
}