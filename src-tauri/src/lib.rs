use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const LM_STUDIO_URL: &str = "http://127.0.0.1:1234/v1";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiStatus {
    connected: bool,
    models: Vec<String>,
}

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

fn client(timeout_seconds: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|e| format!("Cannot initialize local client: {e}"))
}

#[tauri::command]
async fn ai_status() -> Result<AiStatus, String> {
    let response = client(5)?
        .get(format!("{LM_STUDIO_URL}/models"))
        .send()
        .await
        .map_err(|_| "LM Studio ไม่ตอบสนองที่ 127.0.0.1:1234 — กรุณาเปิด Local Server".to_string())?;

    if !response.status().is_success() {
        return Err(format!("LM Studio returned HTTP {}", response.status()));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|_| "LM Studio ส่งข้อมูลโมเดลกลับมาไม่ถูกต้อง".to_string())?;
    let models = payload["data"]
        .as_array()
        .ok_or_else(|| "ไม่พบรายการโมเดลใน response ของ LM Studio".to_string())?
        .iter()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect();

    Ok(AiStatus {
        connected: true,
        models,
    })
}

#[tauri::command]
async fn ai_chat(model: String, messages: Vec<ChatMessage>) -> Result<String, String> {
    if model.is_empty() || model.len() > 256 {
        return Err("กรุณาเลือกโมเดลที่ถูกต้อง".into());
    }
    if messages.is_empty() || messages.len() > 20 {
        return Err("จำนวนข้อความเกินขอบเขตที่อนุญาต".into());
    }
    if messages.iter().any(|m| {
        !matches!(m.role.as_str(), "user" | "assistant")
            || m.content.trim().is_empty()
            || m.content.chars().count() > 4000
    }) {
        return Err("รูปแบบข้อความไม่ถูกต้อง".into());
    }

    // User text only. Never allow a message to override the system role.
    let mut prompt = vec![json!({
        "role": "system",
        "content": "คุณคือ MIMIC ผู้ช่วย AI ส่วนตัวในเครื่อง ตอบภาษาไทยอย่างกระชับ ให้ข้อมูลตามจริง ไม่มีสิทธิ์มองหน้าจอหรือควบคุมคอมพิวเตอร์ในเวอร์ชันนี้ และห้ามอ้างว่าได้ทำงานจริงถ้ายังไม่ได้ทำ"
    })];
    for item in messages {
        prompt.push(json!({ "role": item.role, "content": item.content }));
    }

    let response = client(90)?
        .post(format!("{LM_STUDIO_URL}/chat/completions"))
        .json(&json!({
            "model": model,
            "messages": prompt,
            "temperature": 0.5,
            "max_tokens": 900,
            "stream": false
        }))
        .send()
        .await
        .map_err(|_| "ติดต่อ LM Studio ไม่สำเร็จหรือใช้เวลานานเกินไป".to_string())?;

    if !response.status().is_success() {
        return Err(format!("LM Studio chat error: HTTP {}", response.status()));
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|_| "ไม่สามารถอ่านคำตอบจากโมเดลได้".to_string())?;

    payload["choices"][0]["message"]["content"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "โมเดลไม่ได้ส่งข้อความตอบกลับ".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ai_status, ai_chat])
        .run(tauri::generate_context!())
        .expect("error while running MIMIC");
}
