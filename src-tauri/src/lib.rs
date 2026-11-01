pub mod automation;
pub mod credentials;

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

#[tauri::command]
fn automation_list_windows() -> Result<Vec<automation::WindowInfo>, String> {
    automation::list_windows()
}

#[tauri::command]
fn automation_start_demo() -> Result<(), String> {
    automation::start_demo()
}

#[tauri::command]
fn automation_advance_demo(
    target_window_id: isize,
    permission_granted: bool,
) -> Result<automation::DemoRunResult, String> {
    automation::advance_demo(target_window_id, permission_granted)
}

#[tauri::command]
fn automation_observe_demo(target_window_id: isize) -> Result<automation::DemoObservation, String> {
    automation::observe_demo(target_window_id)
}

#[tauri::command]
fn automation_emergency_stop() -> automation::DemoRunResult {
    automation::emergency_stop()
}

#[tauri::command]
fn automation_reset_emergency_stop() {
    automation::reset_emergency_stop();
}

#[tauri::command]
fn automation_pick_executable() -> Result<Option<String>, String> {
    automation::pick_executable()
}

#[tauri::command]
fn automation_launch_application(executable_path: String) -> Result<u32, String> {
    automation::launch_application(&executable_path)
}

#[tauri::command]
fn automation_focus_window(target_window_id: isize) -> Result<(), String> {
    automation::focus_window(target_window_id)
}

#[tauri::command]
fn automation_select_on_screen(
    target_window_id: Option<isize>,
) -> Result<automation::SelectOnScreenResult, String> {
    automation::select_on_screen(target_window_id)
}

#[tauri::command]
fn automation_execute_step(
    target_window_id: isize,
    step: automation::StepExecutionPayload,
) -> Result<automation::StepExecutionResult, String> {
    automation::execute_step(target_window_id, step)
}

#[tauri::command]
fn automation_authorize_session(
    target_window_id: isize,
    target_exe_path: Option<String>,
) -> Result<automation::NativeAuthorizedSession, String> {
    automation::authorize_session(target_window_id, target_exe_path)
}

#[tauri::command]
fn automation_revoke_session() {
    automation::revoke_session();
}

#[tauri::command]
fn credential_save(target_name: String, username: String, secret: String) -> Result<(), String> {
    credentials::save_credential(&target_name, &username, &secret)
}

#[tauri::command]
fn credential_check(target_name: String) -> Result<credentials::CredentialMetadata, String> {
    credentials::check_credential(&target_name)
}

#[tauri::command]
fn credential_delete(target_name: String) -> Result<(), String> {
    credentials::delete_credential(&target_name)
}

#[tauri::command]
fn credential_list() -> Result<Vec<credentials::CredentialSummary>, String> {
    credentials::list_credentials()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ai_status,
            ai_chat,
            automation_list_windows,
            automation_start_demo,
            automation_advance_demo,
            automation_observe_demo,
            automation_emergency_stop,
            automation_reset_emergency_stop,
            automation_pick_executable,
            automation_launch_application,
            automation_focus_window,
            automation_select_on_screen,
            automation_execute_step,
            automation_authorize_session,
            automation_revoke_session,
            credential_save,
            credential_check,
            credential_delete,
            credential_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MIMIC");
}
