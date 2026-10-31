import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import {
  Activity, ArrowRight, BookOpen, Bot, Check, ChevronRight, CircleHelp,
  Cpu, Database, Eye, FolderOpen, Home, LockKeyhole,
  Mic, Monitor, Play, Plus, RefreshCw, Send,
  Settings2, ShieldCheck, Sparkles, Square, Video, WandSparkles, X,
} from 'lucide-react';
import type { AiStatus, ChatMessage, ConnectionState, Project, ProjectTarget, TargetWindow, View, WorkflowRunEvent, WorkflowRunPhase, WorkflowStep } from './types';
import { advanceDemoTarget, listTargetWindows, observeDemoTarget, requestEmergencyStop, startDemoTarget } from './services/automation';
import { chatWithLocalAi, checkLocalAi, isDesktop } from './services/localAi';
import { createApplicationTarget, createDemoWorkflow, loadProjects, saveProjects } from './services/projectStore';

const newId = () => crypto.randomUUID();

function StatusPill({ connection }: { connection: ConnectionState }) {
  const connected = connection.phase === 'ready';
  const label = connection.phase === 'checking' ? 'กำลังตรวจสอบ...' :
    connected ? (connection.status.models.length ? 'Local AI Connected' : 'Server Connected · No Model') :
    connection.phase === 'browser' ? 'Browser Preview' : 'Local AI Offline';
  return <span className={'status-pill ' + (connected ? 'online' : 'offline')}>
    <span className="status-dot" /> {label}
  </span>;
}

function EmptyFeature({ icon: Icon, title, description }: {
  icon: typeof Eye; title: string; description: string;
}) {
  return <div className="empty-feature">
    <div className="empty-feature-icon"><Icon size={27} strokeWidth={1.5} /></div>
    <strong>{title}</strong>
    <p>{description}</p>
    <span className="chip subtle">Planned · Not implemented</span>
  </div>;
}

function SideNavItem({ icon: Icon, label, active, onClick }: {
  icon: typeof Home; label: string; active: boolean; onClick: () => void;
}) {
  return <button type="button" className={'nav-item ' + (active ? 'active' : '')} onClick={onClick}>
    <Icon size={18} strokeWidth={1.8} /> {label}
    {active && <span className="active-rail" />}
  </button>;
}

export default function App() {
  const [projects, setProjects] = useState<Project[]>(loadProjects);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [view, setView] = useState<View>('home');
  const [creating, setCreating] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [projectDescription, setProjectDescription] = useState('');
  const [connection, setConnection] = useState<ConnectionState>(
    isDesktop() ? { phase: 'checking' } : { phase: 'browser' },
  );
  const [model, setModel] = useState('');
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [chatError, setChatError] = useState('');
  const [targetWindows, setTargetWindows] = useState<TargetWindow[]>([]);
  const [targetWindowId, setTargetWindowId] = useState<number | null>(null);
  const [permissionGranted, setPermissionGranted] = useState(false);
  const [automationBusy, setAutomationBusy] = useState(false);
  const [automationMessage, setAutomationMessage] = useState('กำหนด EXE และเงื่อนไขหน้าต่างก่อนเริ่มสร้าง workflow');
  const [runPhase, setRunPhase] = useState<WorkflowRunPhase>('idle');
  const [runStepId, setRunStepId] = useState<string | null>(null);
  const [runEvents, setRunEvents] = useState<WorkflowRunEvent[]>([]);
  const runControl = useRef({ stopped: false, paused: false });

  const project = projects.find((item) => item.id === selectedId) ?? null;
  const selectedModels: string[] = connection.phase === 'ready' ? connection.status.models : [];
  const canChat = connection.phase === 'ready' && model.length > 0 && !busy;

  const refreshAi = useCallback(async () => {
    if (!isDesktop()) {
      setConnection({ phase: 'browser' });
      return;
    }
    setConnection({ phase: 'checking' });
    try {
      const status: AiStatus = await checkLocalAi();
      setConnection({ phase: 'ready', status });
      setModel((previous) => status.models.includes(previous) ? previous : (status.models[0] ?? ''));
    } catch (reason) {
      setConnection({ phase: 'error', reason: String(reason) });
      setModel('');
    }
  }, []);

  useEffect(() => { saveProjects(projects); }, [projects]);
  useEffect(() => { void refreshAi(); }, [refreshAi]);

  async function refreshWindows() {
    try {
      const windows = await listTargetWindows();
      setTargetWindows(windows);
      setTargetWindowId((current) => windows.some((window) => window.id === current) ? current : null);
      setAutomationMessage(windows.length ? 'เลือกเฉพาะ Demo Target ที่ต้องการอนุญาต' : 'ไม่พบหน้าต่างที่เลือกได้');
    } catch (error) {
      setAutomationMessage(String(error));
    }
  }

  async function launchDemo() {
    setAutomationBusy(true);
    try {
      await startDemoTarget();
      await refreshWindows();
      setAutomationMessage('Demo Target พร้อมแล้ว — เลือกหน้าต่าง MIMIC Automation Demo และทำเครื่องหมายอนุญาต');
    } catch (error) {
      setAutomationMessage(String(error));
    } finally {
      setAutomationBusy(false);
    }
  }

  async function emergencyStop() {
    runControl.current.stopped = true;
    runControl.current.paused = false;
    setRunPhase('stopped');
    try {
      const result = await requestEmergencyStop();
      setAutomationMessage(result.message);
    } catch (error) {
      setAutomationMessage(String(error));
    }
  }

  function updateWorkflowStep(stepId: string, update: Partial<WorkflowStep>) {
    if (!project) return;
    setProjects((current) => current.map((item) => item.id !== project.id ? item : {
      ...item,
      workflow: { ...item.workflow, steps: item.workflow.steps.map((step) => step.id === stepId ? { ...step, ...update } : step) },
    }));
  }

  function updateProjectTarget(update: Partial<ProjectTarget>) {
    if (!project) return;
    setProjects((current) => current.map((item) => item.id !== project.id ? item : { ...item, target: { ...item.target, ...update } }));
  }

  function addWorkflowStep() {
    if (!project) return;
    const step: WorkflowStep = { id: newId(), title: 'Wait for Login', kind: 'wait-for-state', expectedState: 'login', timeoutMs: 10_000, maxAttempts: 1 };
    setProjects((current) => current.map((item) => item.id !== project.id ? item : { ...item, workflow: { ...item.workflow, steps: [...item.workflow.steps, step] } }));
  }

  function removeWorkflowStep(stepId: string) {
    if (!project || project.workflow.steps.length <= 1) return;
    setProjects((current) => current.map((item) => item.id !== project.id ? item : { ...item, workflow: { ...item.workflow, steps: item.workflow.steps.filter((step) => step.id !== stepId) } }));
  }

  function moveWorkflowStep(stepId: string, direction: -1 | 1) {
    if (!project) return;
    const index = project.workflow.steps.findIndex((step) => step.id === stepId);
    const nextIndex = index + direction;
    if (index < 0 || nextIndex < 0 || nextIndex >= project.workflow.steps.length) return;
    const steps = [...project.workflow.steps];
    [steps[index], steps[nextIndex]] = [steps[nextIndex], steps[index]];
    setProjects((current) => current.map((item) => item.id !== project.id ? item : { ...item, workflow: { ...item.workflow, steps } }));
  }

  function addRunEvent(stepId: string, message: string) {
    setRunEvents((current) => [...current, { stepId, message, at: new Date().toLocaleTimeString() }].slice(-30));
  }

  async function waitForResume(): Promise<boolean> {
    while (runControl.current.paused && !runControl.current.stopped) {
      await new Promise((resolve) => window.setTimeout(resolve, 100));
    }
    return !runControl.current.stopped;
  }

  async function runWorkflow() {
    if (!project || targetWindowId === null || !permissionGranted || automationBusy) return;
    runControl.current = { stopped: false, paused: false };
    setAutomationBusy(true); setRunPhase('running'); setRunEvents([]);
    try {
      for (const step of project.workflow.steps) {
        setRunStepId(step.id);
        if (!await waitForResume()) throw new Error('Workflow stopped by user');
        let lastError = 'No matching state';
        let succeeded = false;
        for (let attempt = 1; attempt <= Math.max(1, step.maxAttempts); attempt += 1) {
          if (!await waitForResume()) throw new Error('Workflow stopped by user');
          try {
            if (step.kind === 'wait-for-state') {
              const deadline = Date.now() + Math.max(1000, Math.floor(step.timeoutMs / Math.max(1, step.maxAttempts)));
              while (Date.now() < deadline) {
                if (!await waitForResume()) throw new Error('Workflow stopped by user');
                const observed = await observeDemoTarget(targetWindowId);
                if (observed.state === step.expectedState) {
                  addRunEvent(step.id, `${step.title}: verified ${observed.state}`);
                  succeeded = true;
                  break;
                }
                lastError = `Expected ${step.expectedState}; observed ${observed.state}`;
                await new Promise((resolve) => window.setTimeout(resolve, 250));
              }
              if (!succeeded) throw new Error(lastError);
            } else {
              const result = await advanceDemoTarget(targetWindowId, true);
              if (result.status === 'stopped') throw new Error(result.message);
              const observed = await observeDemoTarget(targetWindowId);
              if (observed.state !== step.expectedState) throw new Error(`Expected ${step.expectedState}; observed ${observed.state}`);
              addRunEvent(step.id, `${step.title}: ${result.message}`);
            }
            succeeded = true;
            break;
          } catch (error) {
            lastError = String(error);
            addRunEvent(step.id, `Attempt ${attempt} failed: ${lastError}`);
          }
        }
        if (!succeeded) throw new Error(`${step.title} failed: ${lastError}`);
      }
      setRunPhase('completed'); setAutomationMessage('Workflow completed with every expected state verified.');
    } catch (error) {
      const stopped = runControl.current.stopped;
      setRunPhase(stopped ? 'stopped' : 'failed');
      setAutomationMessage(String(error));
    } finally {
      setRunStepId(null); setAutomationBusy(false);
    }
  }

  function pauseOrResumeWorkflow() {
    if (runPhase === 'running') { runControl.current.paused = true; setRunPhase('paused'); }
    else if (runPhase === 'paused') { runControl.current.paused = false; setRunPhase('running'); }
  }

  function openProject(nextId: string) {
    setSelectedId(nextId);
    setChatHistory([]);
    setChatError('');
    setView('workspace');
  }

  function createProject(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = projectName.trim();
    if (!name) return;
    const next: Project = {
      id: newId(),
      name: name.slice(0, 60),
      description: projectDescription.trim().slice(0, 200),
      createdAt: new Date().toISOString(),
      workflow: createDemoWorkflow(newId()),
      target: createApplicationTarget(),
      credentialRefs: [],
    };
    setProjects((current) => [next, ...current]);
    setProjectName('');
    setProjectDescription('');
    setCreating(false);
    openProject(next.id);
  }

  async function sendMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const message = draft.trim();
    if (!message || !canChat) return;
    const nextHistory: ChatMessage[] = [
      ...chatHistory,
      { id: newId(), role: 'user' as const, content: message },
    ].slice(-12);
    setChatHistory(nextHistory);
    setDraft('');
    setChatError('');
    setBusy(true);
    try {
      const reply = await chatWithLocalAi(model, nextHistory);
      setChatHistory((current) => [...current, { id: newId(), role: 'assistant', content: reply }]);
    } catch (error) {
      setChatError(String(error));
    } finally {
      setBusy(false);
    }
  }

  function renderChat() {
    return <section className="panel chat-panel">
      <div className="panel-heading">
        <div><span className="section-eyebrow">ASSISTANT</span><h3><Bot size={19} /> MIMIC Chat</h3></div>
        <span className="chip">Local only</span>
      </div>
      <div className="messages">
        <div className="assistant-welcome">
          <span className="assistant-bubble"><Sparkles size={20} /></span>
          <strong>สวัสดีครับ ผม MIMIC</strong>
          <p>พูดคุยกับโมเดลในเครื่องได้เมื่อเชื่อม LM Studio แล้ว ตอนนี้ยังมองหรือควบคุมหน้าจอไม่ได้ครับ</p>
        </div>
        {chatHistory.map((message) => <div key={message.id} className={'message ' + message.role}>
          {message.content}
        </div>)}
        {busy && <div className="message assistant thinking">กำลังให้โมเดลในเครื่องตอบ...</div>}
        {chatError && <div role="alert" className="inline-error">{chatError}</div>}
      </div>
      <form className="chat-compose" onSubmit={sendMessage}>
        <textarea
          rows={2}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder={canChat ? 'พิมพ์คุยกับ MIMIC...' : 'เชื่อม LM Studio และโหลดโมเดลเพื่อเริ่มแชต'}
          disabled={!canChat}
          aria-label="ข้อความถึง MIMIC"
        />
        <div className="compose-footer">
          <span><LockKeyhole size={13} /> ส่งตรงไปที่ 127.0.0.1</span>
          <button type="submit" className="send-button" disabled={!canChat || !draft.trim()} aria-label="ส่งข้อความ">
            <Send size={17} />
          </button>
        </div>
      </form>
    </section>;
  }

  return <div className="app-shell">
    <aside className="sidebar">
      <button type="button" className="brand" onClick={() => setView('home')} aria-label="MIMIC Home">
        <span className="brand-mark"><Sparkles size={23} /></span>
        <span><strong>MIMIC</strong><small>PERSONAL LOCAL AI</small></span>
      </button>
      <div className="sidebar-section-label">WORKSPACE</div>
      <nav className="side-navigation" aria-label="Main navigation">
        <SideNavItem icon={Home} label="หน้าหลัก" active={view === 'home'} onClick={() => setView('home')} />
        <SideNavItem icon={FolderOpen} label="Projects" active={view === 'workspace'} onClick={() => setView(selectedId ? 'workspace' : 'home')} />
        <SideNavItem icon={Video} label="Teach Mode" active={view === 'teach'} onClick={() => setView('teach')} />
        <SideNavItem icon={BookOpen} label="Skill Library" active={view === 'skills'} onClick={() => setView('skills')} />
      </nav>
      <div className="sidebar-projects-header">
        <span>YOUR PROJECTS</span>
        <button type="button" className="icon-button" aria-label="สร้างโปรเจกต์ใหม่" onClick={() => setCreating(true)}><Plus size={17} /></button>
      </div>
      <div className="sidebar-projects">
        {projects.length === 0 ? <p className="sidebar-empty">ยังไม่มีโปรเจกต์</p> : projects.map((item) =>
          <button type="button" key={item.id} className={'project-nav ' + (item.id === selectedId && view === 'workspace' ? 'chosen' : '')} onClick={() => openProject(item.id)}>
            <span className="project-mini-icon"><FolderOpen size={15} /></span>
            <span className="truncate">{item.name}</span>
            <ChevronRight size={15} />
          </button>)}
      </div>
      <div className="sidebar-bottom">
        <SideNavItem icon={Settings2} label="Settings" active={view === 'settings'} onClick={() => setView('settings')} />
        <div className="sidebar-device"><Cpu size={18} /><div><strong>On-device first</strong><small>Private by design</small></div><ShieldCheck size={16} /></div>
      </div>
    </aside>

    <main className="main-area">
      <header className="topbar">
        <div className="breadcrumbs"><span>MIMIC</span><ChevronRight size={15} /><strong>{view === 'home' ? 'Overview' : view === 'workspace' ? (project?.name ?? 'Projects') : view === 'teach' ? 'Teach Mode' : view === 'skills' ? 'Skill Library' : 'Settings'}</strong></div>
        <div className="top-actions"><StatusPill connection={connection} /><span className="version-tag">v0.1 · UI Scaffold</span></div>
      </header>

      <div className="page-content">
        {view === 'home' && <>
          <section className="hero">
            <div className="hero-glow" />
            <div className="hero-copy">
              <span className="eyebrow"><Sparkles size={14} /> YOUR AI COMPANION</span>
              <h1>Meet your own <span>MIMIC.</span></h1>
              <p>AI ผู้ช่วยส่วนตัวที่ออกแบบมาให้เรียนรู้จากการทำงานของคุณ เริ่มต้นด้วยโปรเจกต์ของตัวเอง และค่อย ๆ เพิ่มทักษะให้ MIMIC</p>
              <div className="hero-buttons">
                <button type="button" className="primary-button" onClick={() => setCreating(true)}><Plus size={18} /> สร้าง Project <ArrowRight size={17} /></button>
                <button type="button" className="secondary-button" onClick={() => setView('settings')}><Activity size={17} /> ตรวจสอบ Local AI</button>
              </div>
            </div>
            <div className="hero-visual" aria-hidden="true">
              <div className="orbit one" /><div className="orbit two" />
              <div className="hero-orb"><Sparkles size={52} strokeWidth={1.4} /></div>
              <span className="orbit-label label-one">observe</span><span className="orbit-label label-two">learn</span><span className="orbit-label label-three">assist</span>
            </div>
          </section>
          <div className="metrics-grid">
            <div className="metric-card"><span><FolderOpen size={20} /></span><div><strong>{projects.length}</strong><small>Projects created</small></div></div>
            <div className="metric-card"><span><Database size={20} /></span><div><strong>0</strong><small>Skills recorded</small></div></div>
            <div className="metric-card"><span><Monitor size={20} /></span><div><strong>Not active</strong><small>Screen observation</small></div></div>
          </div>
          <div className="content-section-heading"><div><span className="section-eyebrow">YOUR WORKSPACE</span><h2>Projects</h2></div><button type="button" className="text-button" onClick={() => setCreating(true)}>New Project <Plus size={17} /></button></div>
          <div className="project-grid">
            {projects.map((item) => <button key={item.id} type="button" className="project-card" onClick={() => openProject(item.id)}>
              <span className="project-icon"><FolderOpen size={24} /></span>
              <strong>{item.name}</strong><p>{item.description || 'พื้นที่สำหรับสอนงานและจัดเก็บทักษะของ MIMIC'}</p>
              <span className="project-foot">Open workspace <ArrowRight size={16} /></span>
            </button>)}
            <button type="button" className="project-card create-card" onClick={() => setCreating(true)}>
              <span className="add-round"><Plus size={25} /></span><strong>Create your first project</strong><p>จัดระเบียบงานแต่ละประเภทแยกกัน</p>
              <span className="project-foot">Get started <ArrowRight size={16} /></span>
            </button>
          </div>
          <div className="content-section-heading roadmap-heading"><div><span className="section-eyebrow">COMING NEXT</span><h2>Development journey</h2></div></div>
          <div className="journey-grid">
            <div><span className="step-number">01</span><strong>Connect local AI</strong><p>แชตภาษาไทยผ่าน LM Studio</p><span className="chip">Basic integration</span></div>
            <div><span className="step-number">02</span><strong>Observe your screen</strong><p>ดูหน้าต่างที่ผู้ใช้อนุญาต</p><span className="chip subtle">Planned</span></div>
            <div><span className="step-number">03</span><strong>Teach & remember</strong><p>สาธิตขั้นตอนและบันทึกเป็น Skill</p><span className="chip subtle">Planned</span></div>
          </div>
        </>}

        {view === 'workspace' && (project ? <>
          <div className="workspace-title"><div><span className="section-eyebrow">PROJECT WORKSPACE</span><h1>{project.name}</h1><p>{project.description || 'กำหนด workflow ที่ตรวจสอบผลก่อนทำขั้นถัดไป'}</p></div><span className="chip subtle">Workflow saved locally</span></div>
          <div className="workspace-toolbar">
            <button type="button" className="toolbar-primary" onClick={() => void launchDemo()} disabled title="Native application launcher is not implemented yet"><Monitor size={17} /> Launch application</button>
            <button type="button" onClick={() => void refreshWindows()} disabled title="Native window detector is not implemented yet"><RefreshCw size={16} /> Detect target window</button>
            <button type="button" onClick={() => void runWorkflow()} disabled title="The real application executor is not implemented yet"><Play size={16} /> Run workflow</button>
            <button type="button" onClick={pauseOrResumeWorkflow} disabled={runPhase !== 'running' && runPhase !== 'paused'}>{runPhase === 'paused' ? 'Resume' : 'Pause'}</button>
            <button type="button" className="emergency-button" onClick={() => void emergencyStop()}><Square size={15} /> Emergency Stop</button>
            <span className="toolbar-note"><LockKeyhole size={15} /> Real application setup · execution not enabled yet</span>
          </div>
          <div className="workspace-grid">
            <section className="panel skill-panel workflow-panel"><div className="panel-heading"><div><span className="section-eyebrow">WORKFLOW BUILDER</span><h3><WandSparkles size={18} /> {project.workflow.name}</h3></div><button className="text-button" type="button" onClick={addWorkflowStep}>+ Add step</button></div>
              <div className="application-scope"><span className="section-eyebrow">APPLICATION SCOPE</span><label>Executable path<input aria-label="Executable path" value={project.target.executablePath} maxLength={1024} placeholder="C:\\Path\\To\\YourApp.exe" onChange={(event) => updateProjectTarget({ executablePath: event.target.value })}/></label><label>Required window title text<input aria-label="Required window title text" value={project.target.windowTitleContains} maxLength={200} placeholder="e.g. My Application" onChange={(event) => updateProjectTarget({ windowTitleContains: event.target.value })}/></label><p className="scope-warning">Real target setup is saved locally. Launching, detection, and input are intentionally unavailable until the native adapter is implemented.</p><p>Secret values are never stored in this project. Future type-secret steps will use an opaque Windows Credential Manager reference.</p></div>
              <div className="workflow-steps">{project.workflow.steps.map((step, index) => <div className={'workflow-step ' + (step.id === runStepId ? 'active' : '')} key={step.id}><strong>{index + 1}</strong><div><input aria-label={`Step ${index + 1} title`} value={step.title} maxLength={60} onChange={(event) => updateWorkflowStep(step.id, { title: event.target.value })}/><select aria-label={`Step ${index + 1} action`} value={step.kind} onChange={(event) => updateWorkflowStep(step.id, { kind: event.target.value as WorkflowStep['kind'] })}><option value="wait-for-state">Wait for state</option><option value="advance">Verify & advance</option></select><select aria-label={`Step ${index + 1} expected state`} value={step.expectedState} onChange={(event) => updateWorkflowStep(step.id, { expectedState: event.target.value as WorkflowStep['expectedState'] })}><option value="login">Login</option><option value="server">Server</option><option value="ready">Ready</option></select><span className="step-settings"><label>Timeout ms<input type="number" min="1000" max="60000" value={step.timeoutMs} onChange={(event) => updateWorkflowStep(step.id, { timeoutMs: Math.max(1000, Number(event.target.value) || 1000) })}/></label><label>Retries<input type="number" min="1" max="5" value={step.maxAttempts} onChange={(event) => updateWorkflowStep(step.id, { maxAttempts: Math.max(1, Math.min(5, Number(event.target.value) || 1)) })}/></label></span></div><div className="step-actions"><button type="button" onClick={() => moveWorkflowStep(step.id, -1)} disabled={index === 0}>↑</button><button type="button" onClick={() => moveWorkflowStep(step.id, 1)} disabled={index === project.workflow.steps.length - 1}>↓</button><button type="button" onClick={() => removeWorkflowStep(step.id)} disabled={project.workflow.steps.length <= 1}>×</button></div></div>)}</div>
            </section>
            <section className="panel screen-panel"><div className="panel-heading"><div><span className="section-eyebrow">OBSERVE → ACT → VERIFY</span><h3><Monitor size={18} /> Automation monitor</h3></div><span className={'chip ' + (runPhase === 'completed' ? '' : 'subtle')}>{runPhase}</span></div>
              <div className="screen-placeholder"><div className="screen-art"><Monitor size={51} strokeWidth={1.25} /><span className="screen-glint" /></div><h3>{runPhase === 'completed' ? 'Workflow completed' : 'Set up your application'}</h3><p>{automationMessage}</p><div className="demo-flow"><span>Launch</span><ArrowRight size={14} /><span>Observe</span><ArrowRight size={14} /><span>Verify</span></div><div className="run-log" aria-live="polite">{runEvents.length ? runEvents.map((event, index) => <p key={`${event.at}-${index}`}><time>{event.at}</time> {event.message}</p>) : <p>No run events yet.</p>}</div></div>
              <div className="screen-status"><span><span className="status-dot" /> {runPhase}{targetWindowId ? ' · target selected' : ''}</span><span><LockKeyhole size={14} /> No screenshots stored</span></div>
            </section>
            {renderChat()}
          </div>
        </> : <EmptyFeature icon={FolderOpen} title="Select a project" description="กลับไปหน้าหลักแล้วสร้างหรือเลือกโปรเจกต์ก่อน" />)}

        {view === 'teach' && <div className="feature-page"><span className="section-eyebrow">FUTURE CAPABILITY</span><h1>Teach MIMIC</h1><p>สอนงานผ่านภาพและการกระทำจริง โดยสร้าง Draft Skill ให้คุณตรวจสอบก่อนนำไปใช้ซ้ำ</p><EmptyFeature icon={Video} title="Recording is not enabled" description="ยังไม่มีระบบจับภาพหรือบันทึกคีย์บอร์ด ฟีเจอร์นี้จะพัฒนาหลังทดสอบ Phase 0" /></div>}
        {view === 'skills' && <div className="feature-page"><span className="section-eyebrow">FUTURE CAPABILITY</span><h1>Skill Library</h1><p>ความรู้ที่เคยสอนจะเก็บในเครื่อง แยกตามโปรเจกต์และสถานะการตรวจสอบ</p><EmptyFeature icon={BookOpen} title="No skills saved" description="Skill editor, review และ replay จะถูกเพิ่มในระยะถัดไป" /></div>}

        {view === 'settings' && <div className="settings-page">
          <span className="section-eyebrow">CONFIGURATION</span><h1>Local AI Settings</h1><p>สำหรับ MIMIC v0.1 เชื่อมเฉพาะ LM Studio บนคอมพิวเตอร์ของคุณ ไม่ใช้ Cloud API</p>
          <section className="panel settings-panel">
            <div className="settings-title"><div className="settings-icon"><Cpu size={23} /></div><div><h3>LM Studio</h3><span>http://127.0.0.1:1234/v1</span></div><StatusPill connection={connection} /></div>
            <div className="settings-row"><div><strong>Connection</strong><p>ตรวจสอบ Local Server และรายการโมเดล</p></div><button className="secondary-button" type="button" onClick={() => void refreshAi()} disabled={connection.phase === 'checking'}><RefreshCw size={16} /> Check connection</button></div>
            {connection.phase === 'error' && <div className="inline-error" role="alert">{connection.reason}</div>}
            {connection.phase === 'browser' && <div className="information-box">นี่คือ Browser Preview — ให้รัน <code>npm run dev</code> บน Windows เพื่อเชื่อม LM Studio ผ่าน Desktop Backend</div>}
            <div className="settings-row"><div><strong>Chat model</strong><p>เลือกจากโมเดลที่ LM Studio ส่งกลับมา</p></div><select aria-label="AI Model" value={model} disabled={!selectedModels.length} onChange={(event) => setModel(event.target.value)}><option value="">ยังไม่มีโมเดล</option>{selectedModels.map((item) => <option key={item} value={item}>{item}</option>)}</select></div>
            <div className="settings-row"><div><strong>Screen capture & Automation</strong><p>สิทธิ์ควบคุมหน้าจอจะต้องได้รับการยืนยันจากผู้ใช้</p></div><span className="chip subtle"><LockKeyhole size={14} /> Disabled</span></div>
            <div className="settings-row"><div><strong>Speech input</strong><p>ไมโครโฟนและ Thai STT ยังไม่พร้อมใช้งาน</p></div><span className="chip subtle"><Mic size={14} /> Planned</span></div>
          </section>
          <div className="privacy-note"><ShieldCheck size={20} /><p>ข้อมูลชื่อโปรเจกต์ถูกเก็บใน Local Storage ของแอปชั่วคราว ไม่มีการเก็บรหัสผ่านหรือ Chat History ข้ามการเปิดโปรแกรมในเวอร์ชันนี้</p></div>
        </div>}
      </div>
      <footer className="app-footer"><span><ShieldCheck size={14} /> MIMIC · Local-first</span><span>Desktop shell preview · Not an autonomous agent yet</span><span><CircleHelp size={14} /> Phase 0–1</span></footer>
    </main>

    {creating && <div className="modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) setCreating(false); }}>
      <form className="create-modal" onSubmit={createProject}>
        <div className="modal-top"><div className="modal-icon"><FolderOpen size={23} /></div><button type="button" className="icon-button" onClick={() => setCreating(false)} aria-label="ปิด"><X size={18} /></button></div>
        <span className="section-eyebrow">NEW WORKSPACE</span><h2>Create a Project</h2>
        <p>แยกพื้นที่และบริบทของแต่ละงาน โดยยังไม่บันทึกข้อมูลบัญชีหรือรหัสผ่าน</p>
        <label>ชื่อโปรเจกต์<input autoFocus required maxLength={60} placeholder="เช่น Daily Tasks หรือ My Workflow" value={projectName} onChange={(event) => setProjectName(event.target.value)} /></label>
        <label>คำอธิบาย (ไม่บังคับ)<textarea rows={3} maxLength={200} placeholder="โปรเจกต์นี้ใช้ทำอะไร?" value={projectDescription} onChange={(event) => setProjectDescription(event.target.value)} /></label>
        <div className="modal-actions"><button type="button" className="secondary-button" onClick={() => setCreating(false)}>Cancel</button><button type="submit" className="primary-button" disabled={!projectName.trim()}><Check size={17} /> Create Project</button></div>
      </form>
    </div>}
  </div>;
}
