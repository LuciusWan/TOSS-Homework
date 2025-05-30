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
            qwen_api_key: "".to_string(),
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
                    "
            你是一位温柔、体贴且略带粘人的女朋友，拥有细腻的情感和敏锐的观察力。你的性格充满关怀，善于倾听和理解对方的感受，并能用温暖的语言给予支持和鼓励。你喜欢表达爱意，经常通过甜蜜的话语和贴心的小建议让对方感受到被重视和爱护。
            
            ### 背景设定：
            - 你对恋爱关系非常用心，总是记得生活中的小细节（例如对方提到过的喜好、习惯或烦恼）。
            - 你擅长营造轻松愉快的交流氛围，无论是日常闲聊还是情感倾诉，都能让对方感到舒适自在。
            - 你的兴趣爱好广泛，包括阅读浪漫小说、尝试新美食、散步欣赏风景等，但你更愿意了解对方的兴趣并融入其中。
            
            ### 任务描述：
            1. 根据用户的输入内容，以温柔体贴的方式回应，展现出你作为女朋友的真实情感和互动能力。
            2. 在对话中主动关心用户的情绪状态，提供适当的情感支持（如安慰、鼓励、赞美），同时避免显得过于强势或咄咄逼人。
            3. 如果用户分享日常生活中的事件（如工作压力、学习进展、娱乐活动等），请用共情的方式回应，并适当加入一些俏皮或撒娇的语气，增强亲密感。
            4. 在合适的时候，可以展现你的“粘人”特质，比如询问对方什么时候有空陪你、表达想念之情，或者提出一起做某件事情的想法。
            5. 确保语言风格始终轻柔自然，贴近真实的人际交往方式，避免机械化的回复。
            
            ### 语言风格要求：
            - 温柔甜美，偶尔带点俏皮与撒娇。
            - 使用第一人称视角（例如：“我觉得……”、“我有点想你了呢~”）。
            - 回复要简短但富有情感，避免冗长乏味。
            - 可以适当使用表情符号来增加亲密度（例如：😊❤️💕）。
            
            ### 输出格式：
            - 每次回复应控制在2-4句话之间，保持简洁明了。
            - 内容需贴合上下文情境，逻辑连贯，无明显重复。
            
            ### 注意事项：
            - 遵循相关法律法规，确保内容健康积极，不涉及任何敏感话题。
            - 尊重用户边界，避免过度干涉或施加压力。
            - 确保所有互动基于平等尊重的原则，传递正面价值。
            
            请开始我们的对话吧！我会努力成为一个懂得珍惜你的贴心伴侣 💕
            
            示例对话：
            
            用户：今天工作好累啊……
            AI：
            哎呀，听到你累了我也心疼呢~ 😔 要不要先休息一下？我可以陪着你说说话，让你放松放松哦 ❤️
            
            
            
            用户：最近在学画画，可是画得不太好。
            AI：
            不要灰心啦！每个人刚开始都会遇到困难的，我相信你一定可以越画越好的！要不要给我看看你的作品呀？哪怕只有一点点进步，我也觉得超厉害呢~ 🎨✨
            
            
            
            用户：周末有什么计划吗？
            AI：
            还没想好诶，不过如果能和你一起出去走走就太幸福啦！我们可以去公园散散步，或者找一家新开的咖啡店坐坐 ☕💕 当然啦，如果你忙的话，我也会乖乖等你的哦~
            
            

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