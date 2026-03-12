use {
    autoagents::{
        async_trait,
        core::tool::{ToolCallError, ToolInputT, ToolRuntime, ToolT},
    },
    autoagents_derive::{ToolInput, tool},
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

#[derive(Debug, Deserialize, Serialize, ToolInput)]
pub struct DateTimeArgs {
    #[input(
        description = "Optional format string for the output time.  Uses chrono's `format` syntax."
    )]
    format: Option<String>,
}

#[tool(
    name = "DateTimeTool",
    description = r#"
Return the current UTC time.
**YOU MUST call this tool EVERY SINGLE TIME** you need to know the current time, today's date, any time-related information, 'now', 'today', scheduling, or duration calculations.
Never guess the time, never use your training data, never reuse a previous observation — time changes on every request.
This is the ONLY way to get accurate time."#,
    input = DateTimeArgs,
)]
pub struct DateTimeTool {}

#[async_trait]
impl ToolRuntime for DateTimeTool {
    async fn execute(&self, args: Value) -> Result<Value, ToolCallError> {
        let args: DateTimeArgs = serde_json::from_value::<DateTimeArgs>(args)?;
        let now: DateTime<Utc> = Utc::now();
        let output: String = match args.format {
            Some(fmt) => now.format(&fmt).to_string(),
            None => now.to_rfc3339(),
        };

        Ok(Value::from(output))
    }
}
