use openai_api_rust::completions::*;
use openai_api_rust::*;
use openai_harmony::chat::{Author, Conversation, Message, Role, SystemContent};
use openai_harmony::{HarmonyEncodingName, load_harmony_encoding};

use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

struct Agent {
    encoding: openai_harmony::HarmonyEncoding,
    openai: OpenAI,
    conversation: Conversation,
}

impl Agent {
    fn new() -> anyhow::Result<Self> {
        let encoding = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)?;
        let auth = Auth::new("not-needed");
        let openai = OpenAI::new(auth, "http://localhost:1234/v1/");

        // Create initial conversation with system and developer messages
        let system_content = SystemContent::new()
            .with_model_identity("You are ChatGPT, a large language model trained by OpenAI.")
            .with_knowledge_cutoff("2024-06")
            .with_conversation_start_date("2025-01-08")
            .with_reasoning_effort(openai_harmony::chat::ReasoningEffort::High)
            .with_required_channels(["analysis", "commentary", "final"]);

        let developer_message = r#"# Instructions

You are a helpful coding assistant that can read, list, and edit files. Be concise and helpful.

Calls to these tools must go to the commentary channel: 'functions'.

# Tools

## functions

namespace functions {

// Read the contents of a given relative file path
type read_file = (_: {
// The relative path of a file in the working directory
path: string,
}) => any;

// List files and directories at a given path
type list_files = (_: {
// Optional relative path to list files from. Defaults to current directory if not provided
path?: string,
}) => any;

// Make edits to a text file
type edit_file = (_: {
// The path to the file
path: string,
// Text to search for - must match exactly
old_str: string,
// Text to replace old_str with
new_str: string,
}) => any;

} // namespace functions"#;

        let conversation = Conversation::from_messages([
            Message::from_role_and_content(Role::System, system_content),
            Message::from_role_and_content(Role::Developer, developer_message),
        ]);

        Ok(Agent {
            encoding,
            openai,
            conversation,
        })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        println!("Chat with the coding assistant (use 'exit' to quit)");

        loop {
            print!("\n\x1b[94mYou\x1b[0m: ");
            io::stdout().flush()?;

            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => return Err(e.into()),
            }
            let input = input.trim();

            if input == "exit" {
                break;
            }

            if input.is_empty() {
                continue;
            }

            // Add user message to conversation
            self.conversation
                .messages
                .push(Message::from_role_and_content(Role::User, input));

            // Run inference loop until we get a final response
            loop {
                match self.run_inference()? {
                    InferenceResult::FinalResponse(response) => {
                        println!("\x1b[93mAssistant\x1b[0m: {}", response);
                        break;
                    }
                    InferenceResult::ToolCall {
                        tool_name,
                        arguments,
                    } => {
                        println!("\x1b[92mtool\x1b[0m: {}({})", tool_name, arguments);
                        let result = self.execute_tool(&tool_name, &arguments)?;

                        // Add tool result to conversation
                        let tool_message = Message::from_author_and_content(
                            Author::new(Role::Tool, tool_name.clone()),
                            result,
                        )
                        .with_recipient("assistant")
                        .with_channel("commentary");

                        self.conversation.messages.push(tool_message);
                    }
                }
            }
        }

        Ok(())
    }

    fn run_inference(&mut self) -> anyhow::Result<InferenceResult> {
        // Render conversation to tokens
        let tokens = self.encoding.render_conversation_for_completion(
            &self.conversation,
            Role::Assistant,
            None,
        )?;
        let prompt = self.encoding.tokenizer().decode_utf8(&tokens)?;

        // Make API request
        let body = CompletionsBody {
            model: "openai/gpt-oss-20b".to_string(),
            prompt: Some(vec![prompt]),
            max_tokens: Some(2048),
            temperature: Some(0.7),
            top_p: None,
            n: Some(1),
            stream: Some(false),
            logprobs: None,
            echo: Some(false),
            stop: Some(vec!["<|return|>".to_string(), "<|call|>".to_string()]),
            presence_penalty: None,
            frequency_penalty: None,
            best_of: None,
            logit_bias: None,
            user: None,
            suffix: None,
        };

        let response = self
            .openai
            .completion_create(&body)
            .map_err(|e| anyhow::anyhow!("API error: {:?}", e))?;

        if let Some(choice) = response.choices.get(0) {
            if let Some(response_text) = &choice.text {
                // Use harmony library to parse the response properly
                // If the response looks like a tool call but is missing <|call|>, add it
                let complete_response = if response_text.contains("to=functions.")
                    && !response_text.contains("<|call|>")
                {
                    format!("{}<|call|>", response_text)
                } else {
                    response_text.clone()
                };

                let response_tokens = self
                    .encoding
                    .tokenizer()
                    .encode(&complete_response, &std::collections::HashSet::new())
                    .0;

                // println!("DEBUG: Raw response: {}", response_text);
                // println!("DEBUG: Complete response: {}", complete_response);
                match self
                    .encoding
                    .parse_messages_from_completion_tokens(response_tokens, Some(Role::Assistant))
                {
                    Ok(messages) => {
                        // println!("DEBUG: Parsed {} messages", messages.len());
                        // for (i, msg) in messages.iter().enumerate() {
                        //     println!("DEBUG: Message {}: channel={:?}, recipient={:?}", i, msg.channel, msg.recipient);
                        // }
                        // Add parsed messages to conversation
                        for message in &messages {
                            self.conversation.messages.push(message.clone());
                        }

                        // Check for tool calls
                        for message in &messages {
                            if let Some(recipient) = &message.recipient {
                                if recipient.starts_with("functions.") {
                                    let tool_name =
                                        recipient.strip_prefix("functions.").unwrap_or(recipient);
                                    // Get the content as the arguments
                                    let arguments = if let Some(content) = message.content.get(0) {
                                        if let openai_harmony::chat::Content::Text(text_content) =
                                            content
                                        {
                                            text_content.text.clone()
                                        } else {
                                            "{}".to_string()
                                        }
                                    } else {
                                        "{}".to_string()
                                    };

                                    return Ok(InferenceResult::ToolCall {
                                        tool_name: tool_name.to_string(),
                                        arguments,
                                    });
                                }
                            }
                        }

                        // Check for final responses
                        for message in &messages {
                            if let Some(channel) = &message.channel {
                                if channel == "final" {
                                    if let Some(content) = message.content.get(0) {
                                        if let openai_harmony::chat::Content::Text(text_content) =
                                            content
                                        {
                                            return Ok(InferenceResult::FinalResponse(
                                                text_content.text.clone(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // println!("DEBUG: Harmony parsing failed, using manual parsing");

                        // Fallback to manual parsing if harmony parsing fails
                        let assistant_message =
                            Message::from_role_and_content(Role::Assistant, response_text);
                        self.conversation.messages.push(assistant_message);

                        // Check for tool calls manually
                        if let Some((tool_name, arguments)) = self.parse_tool_call(response_text) {
                            // println!("DEBUG: Found tool call: {} with args: {}", tool_name, arguments);
                            return Ok(InferenceResult::ToolCall {
                                tool_name,
                                arguments,
                            });
                        }

                        if let Some(final_content) = self.extract_final_content(response_text) {
                            return Ok(InferenceResult::FinalResponse(final_content));
                        }
                        return Ok(InferenceResult::FinalResponse(response_text.clone()));
                    }
                }
            }
        }

        anyhow::bail!("No response received from API")
    }

    fn parse_tool_call(&self, response_text: &str) -> Option<(String, String)> {
        // Look for pattern: <|channel|>commentary to=functions.TOOL_NAME <|constrain|>json<|message|>ARGS<|call|>
        if let Some(to_start) = response_text.find("to=functions.") {
            let tool_start = to_start + "to=functions.".len();
            if let Some(tool_end) = response_text[tool_start..].find(' ') {
                let tool_name = &response_text[tool_start..tool_start + tool_end];

                // Find the arguments after the LAST <|message|> (for the tool call)
                if let Some(msg_start) = response_text.rfind("<|message|>") {
                    let args_start = msg_start + "<|message|>".len();
                    // Since <|call|> is a stop token, it won't be in the response
                    // Just take everything from <|message|> to the end
                    let arguments = &response_text[args_start..].trim();

                    // Debug output
                    // println!("DEBUG: Extracted tool: {}, args: '{}'", tool_name, arguments);

                    return Some((tool_name.to_string(), arguments.to_string()));
                }
            }
        }
        None
    }

    fn extract_final_content(&self, response_text: &str) -> Option<String> {
        if let Some(final_start) = response_text.find("<|channel|>final<|message|>") {
            let final_content_start = final_start + "<|channel|>final<|message|>".len();
            let final_content =
                if let Some(end_pos) = response_text[final_content_start..].find("<|") {
                    &response_text[final_content_start..final_content_start + end_pos]
                } else {
                    &response_text[final_content_start..]
                };
            return Some(final_content.trim().to_string());
        }
        None
    }

    fn execute_tool(&self, tool_name: &str, arguments: &str) -> anyhow::Result<String> {
        let args: Value = match serde_json::from_str(arguments) {
            Ok(args) => args,
            Err(_) => {
                // If JSON parsing fails, try to handle it gracefully
                eprintln!("Warning: Failed to parse arguments as JSON: {}", arguments);
                return Ok("Invalid JSON arguments".to_string());
            }
        };

        match tool_name {
            "read_file" => self.read_file(&args),
            "list_files" => self.list_files(&args),
            "edit_file" => self.edit_file(&args),
            _ => anyhow::bail!("Unknown tool: {}", tool_name),
        }
    }

    fn read_file(&self, args: &Value) -> anyhow::Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
        let content = fs::read_to_string(path)?;
        Ok(content)
    }

    fn list_files(&self, args: &Value) -> anyhow::Result<String> {
        let path = args["path"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(".");
        let entries = fs::read_dir(path)?;

        let mut files = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_dir() {
                files.push(format!("{}/", name));
            } else {
                files.push(name);
            }
        }

        files.sort();
        Ok(serde_json::to_string(&files)?)
    }

    fn edit_file(&self, args: &Value) -> anyhow::Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
        let old_str = args["old_str"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing old_str argument"))?;
        let new_str = args["new_str"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing new_str argument"))?;

        if old_str == new_str {
            anyhow::bail!("old_str and new_str must be different");
        }

        // Read file or create if it doesn't exist and old_str is empty
        let content = if Path::new(path).exists() {
            fs::read_to_string(path)?
        } else if old_str.is_empty() {
            String::new()
        } else {
            anyhow::bail!("File {} does not exist", path);
        };

        let new_content = if old_str.is_empty() {
            new_str.to_string()
        } else {
            if !content.contains(old_str) {
                anyhow::bail!("old_str not found in file");
            }
            content.replace(old_str, new_str)
        };

        // Create directory if needed
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, new_content)?;
        Ok("OK".to_string())
    }
}

#[derive(Debug)]
enum InferenceResult {
    FinalResponse(String),
    ToolCall {
        tool_name: String,
        arguments: String,
    },
}

fn main() -> anyhow::Result<()> {
    let mut agent = Agent::new()?;
    agent.run()
}
