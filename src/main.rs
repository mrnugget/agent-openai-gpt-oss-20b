use openai_api_rust::*;
use openai_api_rust::chat::*;
use std::env;

fn main() {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run \"your message here\"");
        std::process::exit(1);
    }
    let user_message = &args[1];

    // Create OpenAI client pointing to local LM Studio server
    // Since LM Studio doesn't require API key, we can use a dummy one
    let auth = Auth::new("not-needed");
    let openai = OpenAI::new(auth, "http://localhost:1234/v1/");

    // Create chat completion request
    let body = ChatBody {
        model: "openai/gpt-oss-20b".to_string(),
        max_tokens: Some(512),
        temperature: Some(0.7),
        top_p: None,
        n: Some(1),
        stream: Some(false),
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        messages: vec![Message {
            role: Role::User,
            content: user_message.clone(),
        }],
    };

    // Make the API request
    match openai.chat_completion_create(&body) {
        Ok(response) => {
            if let Some(choice) = response.choices.get(0) {
                if let Some(message) = &choice.message {
                    println!("Assistant: {}", message.content);
                } else {
                    eprintln!("No message content found in response");
                }
            } else {
                eprintln!("No choices returned in response");
            }
        }
        Err(e) => {
            eprintln!("Error calling OpenAI API: {:?}", e);
        }
    }
}
