use openai_api_rust::*;
use openai_api_rust::completions::*;
use openai_harmony::chat::{Conversation, Message, Role, SystemContent};
use openai_harmony::{load_harmony_encoding, HarmonyEncodingName};
use std::env;

fn main() -> anyhow::Result<()> {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run \"your message here\"");
        std::process::exit(1);
    }
    let user_message = &args[1];

    // Load harmony encoding
    let encoding = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)?;

    // Create conversation with proper harmony format
    let system_content = SystemContent::new()
        .with_model_identity("You are ChatGPT, a large language model trained by OpenAI.")
        .with_knowledge_cutoff("2024-06")
        .with_conversation_start_date("2025-01-08")
        .with_reasoning_effort(openai_harmony::chat::ReasoningEffort::High)
        .with_required_channels(["analysis", "commentary", "final"]);

    let developer_message = "# Instructions\n\nBe helpful and concise.".to_string();

    let conversation = Conversation::from_messages([
        Message::from_role_and_content(Role::System, system_content),
        Message::from_role_and_content(Role::Developer, developer_message),
        Message::from_role_and_content(Role::User, user_message.clone()),
    ]);

    // Render conversation to tokens
    let tokens = encoding.render_conversation_for_completion(&conversation, Role::Assistant, None)?;

    // Convert tokens to string for the API call
    let prompt = encoding.tokenizer().decode_utf8(&tokens)?;

    // Create OpenAI client pointing to local LM Studio server
    let auth = Auth::new("not-needed");
    let openai = OpenAI::new(auth, "http://localhost:1234/v1/");

    // Make API request using completions endpoint (not chat) since we're sending raw tokens
    let body = CompletionsBody {
        model: "openai/gpt-oss-20b".to_string(),
        prompt: Some(vec![prompt]),
        max_tokens: Some(512),
        temperature: Some(0.7),
        top_p: None,
        n: Some(1),
        stream: Some(false),
        logprobs: None,
        echo: Some(false),
        stop: Some(vec![
            "<|return|>".to_string(),
            "<|call|>".to_string(),
        ]),
        presence_penalty: None,
        frequency_penalty: None,
        best_of: None,
        logit_bias: None,
        user: None,
        suffix: None,
    };

    // Make the API request
    match openai.completion_create(&body) {
        Ok(response) => {
            if let Some(choice) = response.choices.get(0) {
                let response_text = &choice.text;
                
                // Extract final channel content using simple string parsing for now
                if let Some(response_text) = response_text {
                    // Look for the final channel content pattern
                    if let Some(final_start) = response_text.find("<|channel|>final<|message|>") {
                        let final_content_start = final_start + "<|channel|>final<|message|>".len();
                        let final_content = if let Some(end_pos) = response_text[final_content_start..].find("<|") {
                            &response_text[final_content_start..final_content_start + end_pos]
                        } else {
                            &response_text[final_content_start..]
                        };
                        println!("Assistant: {}", final_content.trim());
                    } else {
                        // Fallback: just print the raw response
                        println!("Assistant: {}", response_text);
                    }
                } else {
                    eprintln!("No response text received");
                }
            } else {
                eprintln!("No choices returned in response");
            }
        }
        Err(e) => {
            eprintln!("Error calling OpenAI API: {:?}", e);
        }
    }

    Ok(())
}
