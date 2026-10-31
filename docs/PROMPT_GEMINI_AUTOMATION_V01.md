# Prompt for Gemini Coding Agent — MIMIC v0.1

Copy the contents below into Gemini / your coding agent while it has access to a working local clone of this repository and a Windows 11 computer.

---

คุณเป็น Senior Windows Desktop Engineer และ Coding Agent ของโปรเจกต์ **MIMIC** บน GitHub:

https://github.com/TephudsadinAIS/MIMIC

## เป้าหมายงานนี้

**พัฒนา MIMIC v0.1 เป็นโปรแกรม Windows Automation แบบไม่ใช้ AI ก่อน** จากโค้ดจริงใน Repository ปัจจุบัน: Tauri 2 + React + TypeScript + Rust

อย่าสร้างแอปใหม่ตั้งแต่ศูนย์โดยไม่จำเป็น และอย่าทิ้ง UI ที่มีอยู่ แต่เปลี่ยนได้หากพิสูจน์แล้วว่ามีข้อจำกัดทางเทคนิคจริง

ตัวอย่างงานระยะยาว: เปิด EXE → รอ Login → กรอกบัญชีอย่างปลอดภัย → คลิกเข้าเกม → เลือกเซิร์ฟเวอร์ → ตรวจสอบว่าเข้าโลกเกมแล้ว → จากนั้นจึงเริ่มบัญชีถัดไปตามลำดับ อย่างไรก็ตาม **ให้พิสูจน์ระบบกับแอปทดลองที่ควบคุมได้ก่อน ห้ามสมมุติว่าระบบของเกมจริงอนุญาตหรือใช้งานได้** ไม่พยายามหลบ anti-cheat หรือระบบป้องกันใด ๆ

## อ่านไฟล์เหล่านี้ให้ครบก่อนแก้โค้ด

1. \`AGENTS.md\`
2. \`docs/AUTOMATION_MVP_V01.md\` (**ขอบเขตงานล่าสุด ให้ความสำคัญสูงสุด**)
3. \`docs/WORKFLOW_CONTRACT_V01.md\`
4. \`docs/ARCHITECTURE.md\`
5. \`docs/IMPLEMENTATION_STATUS.md\`
6. \`MIMIC_PROJECT_OVERVIEW.md\` (เป้าหมายระยะยาว ไม่ใช่ขอบเขต MVP)

## สิ่งที่ต้องลงมือทำ

**A. ตรวจสอบโค้ดเดิมให้ใช้งานได้จริง**
- Clone / pull branch ล่าสุด จากนั้นตรวจโครงสร้างและสิ่งที่ implement แล้ว
- ตรวจ Node, npm, Rust, MSVC, WebView2 และ Tauri requirements ใน Windows
- รัน \`npm install\`, \`npm run check\`, \`npm run build:web\`, \`npm run dev\`
- แก้ปัญหา build/runtime จริงที่พบ โดยอธิบายเหตุผลและรายงานผลทดสอบ
- ไม่ต้องเปิด LM Studio; หากปิดอยู่ MIMIC ต้องใช้งานฟังก์ชัน Automation ได้

**B. สร้างการทดลอง Windows Automation ที่ปลอดภัย**
- ทำ Test Window/Demo App (เช่น Login → Select Server → Ready) โดยไม่ใช้บัญชีจริง
- เพิ่ม Native Window Enumeration + เลือก target process/window แบบชัดเจน
- ตรวจ UIA (Windows UI Automation) ก่อน; ถ้าใช้ไม่ได้ ให้ทดลอง Window-scoped visual matching อย่างถูกต้อง
- เพิ่มกลไกแสดงสิทธิ์, ยืนยันก่อนส่ง input, และ Emergency Stop ที่ไม่ต้องผ่าน AI
- สร้าง 1 automation: เปิด Demo App → รอหน้าที่คาดหมาย → กดปุ่มในหน้าต่างที่ถูกต้อง → ตรวจผลลัพธ์
- หากตรวจจับไม่เจอ/เกิด Error/Timeout ต้องหยุดและรายงาน ห้ามคลิกสุ่มหรือวนไม่รู้จบ

**C. ขยายเป็น Workflow Builder ที่ใช้ซ้ำได้**
- Project เลือก EXE และ Windows target; UI สำหรับ Steps + Wait Condition + Action + Verification
- เพิ่ม Run/Pause/Resume/Stop และ Event/Run Monitor พร้อมเหตุผล
- ใช้ typed model + runtime validation + safe native command boundaries
- บันทึก Workflow/Project ใน app-local persistent storage, อ่านกลับได้หลัง restart
- ไม่ hardcode สถานะของเกมใดเป็นพิเศษ และไม่เก็บ credentials ใน JSON, localStorage หรือ Log
- หลังผ่าน 1 workflow ให้เพิ่มหลาย Profile **แบบ Sequential** โดยไม่จำเป็นต้องเริ่ม 6 บัญชีทันที
- ทำ credential integration ผ่าน Windows Credential Manager ก่อนใช้ข้อมูลจริง

**D. UI และการทดสอบ**
- รักษาธีม MIMIC ที่มีอยู่ ปรับ UI ให้ใช้งานจริง สวย สะอาด ไม่ซับซ้อน
- ส่วนที่ยังไม่พร้อมให้แสดง Disabled/Coming Soon ตามจริง
- เขียน unit tests ให้ workflow state machine, timeout, cancellation, failed postcondition และ basic persistence
- ทดสอบกับ Demo App ด้วยข้อมูลจำลองก่อน และสรุปผลอย่างตรงไปตรงมา
- อัปเดตเอกสารสถานะหลังทำเสร็จ หากยัง build/run ไม่ได้ ให้บันทึกข้อผิดพลาดจริง

## สิ่งที่ห้ามทำ

- ไม่ต้องเชื่อม AI, Vision Model, LM Studio, Speech หรือระบบสอนงานด้วยการอัดจอในงานนี้
- ห้ามให้โปรแกรมควบคุมหน้าต่างอื่นนอกจากที่ผู้ใช้เลือก
- ห้าม hardcode รหัสผ่าน บันทึกคีย์ที่พิมพ์/ภาพหน้าจอโดยไม่ได้รับอนุญาต ห้ามเก็บ secrets ลง Git
- ห้ามหลบ CAPTCHA, anti-cheat หรือดัดแปลง/ฉีดโค้ดเข้าโปรเซสของเกม
- ห้ามอ้างว่าทดสอบผ่าน ถ้ายังไม่ได้รันจริงบน Windows
- ห้าม rewrite ทั้งโปรเจกต์เพียงเพื่อเปลี่ยน framework โดยไม่มีหลักฐาน
- อย่าทำทุกฟีเจอร์ใหญ่พร้อมกัน ให้ deliver เป็น milestone ที่ทดสอบได้

## รูปแบบทำงานที่ต้องการ

1. เริ่มด้วยสรุปสภาพ Repo ที่พบ และแผน 3–4 milestones สั้น ๆ
2. ลงมือแก้โค้ดจริง **Milestone A ก่อน** ให้เปิด MIMIC Desktop และผ่าน Observe → Act → Verify บน Demo App
3. รันทดสอบและแก้ปัญหาจนผ่านหรืออธิบาย blocker
4. เมื่อผ่านแล้วต่อ Milestone B (Workflow Builder + Persistence) และ C เฉพาะส่วนที่พิสูจน์ได้
5. Commit และ Push การเปลี่ยนแปลงขึ้น Repository เมื่อมีสิทธิ์และผู้ใช้อนุญาต; ไม่ force-push, ไม่ commit secrets
6. ตอนจบส่ง **ไฟล์ที่แก้, วิธีทดสอบ, หลักฐานผลลัพธ์, สิ่งที่ยังทำไม่ได้ และขั้นตอนเปิดโปรแกรมบน Windows**

## เกณฑ์สำเร็จขั้นต่ำ

ผู้ใช้เปิด MIMIC ได้บน Windows 11, สร้าง Project, กำหนด Workflow, กด Run กับ Demo App ที่เลือกไว้, โปรแกรมทำขั้นตอนและตรวจสอบหน้าจอ/สถานะได้โดยไม่เปิด LM Studio, กด Stop ได้เสมอ, ปิดแล้วเปิดใหม่ยังพบ Workflow และไม่พบ credentials ในที่เก็บข้อมูลที่ไม่ปลอดภัย

หากงานทั้งหมดไม่เสร็จในรอบเดียว ให้ส่งมอบ Milestone A ที่ **ทำงานจริงและทดสอบได้** ก่อน อย่าแสดงงานหลอกหรือปุ่มที่ไม่มีฟังก์ชัน.

---
