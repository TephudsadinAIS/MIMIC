import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import {
  Activity, ArrowRight, BookOpen, Bot, Check, CheckCircle2, ChevronRight, CircleHelp,
  Cpu, Crosshair, Download, Eye, FolderOpen, Home, KeyRound, ListOrdered, LockKeyhole,
  Monitor, Pause, Play, Plus, RefreshCw, Send,
  Settings2, ShieldCheck, SkipForward, Sparkles, Square, Trash2, Upload, Users, Video, WandSparkles, X,
} from 'lucide-react';
import type {
  AccountProfile, AccountQueueItem, ActionType, AiStatus, ChatMessage, ConnectionState, Locator,
  Project, ProjectTarget, SelectOnScreenResult, StepExecutionResult, TargetWindow,
  View, WorkflowRunEvent, WorkflowRunPhase, WorkflowStep,
} from './types';
import {
  authorizeSession, executeStep, focusWindow, launchApplication, listTargetWindows,
  pickExecutable, requestEmergencyStop, resetEmergencyStop, revokeSession,
  selectOnScreen, startDemoTarget,
} from './services/automation';
import { chatWithLocalAi, checkLocalAi, isDesktop } from './services/localAi';
import {
  createApplicationTarget, createDemoWorkflow, exportProjectBundle,
  importProjectBundle, loadProjects, saveProjects,
} from './services/projectStore';
import { checkCredential, deleteCredential, saveCredential } from './services/credentialService';

const newId = () => crypto.randomUUID();

function StatusPill({ connection }: { connection: ConnectionState }) {
  const connected = connection.phase === 'ready';
  const label = connection.phase === 'checking' ? 'กำลังตรวจสอบ...' :
    connected ? (connection.status.models.length ? 'Local AI Connected' : 'Server Connected · No Model') :
    connection.phase === 'disabled' ? 'Local AI Off' :
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
  const [aiEnabled, setAiEnabled] = useState(true);
  const [model, setModel] = useState('');
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [chatError, setChatError] = useState('');

  // Target Application & Window State
  const [targetWindows, setTargetWindows] = useState<TargetWindow[]>([]);
  const [targetWindowId, setTargetWindowId] = useState<number | null>(null);
  const [permissionGranted, setPermissionGranted] = useState(false);
  const [automationBusy, setAutomationBusy] = useState(false);
  const [automationMessage, setAutomationMessage] = useState('พร้อมใช้งาน: เลือก Application Scope และหน้าต่างเป้าหมาย');
  const [runPhase, setRunPhase] = useState<WorkflowRunPhase>('idle');
  const [runStepId, setRunStepId] = useState<string | null>(null);
  const [runEvents, setRunEvents] = useState<WorkflowRunEvent[]>([]);
  const runControl = useRef({ stopped: false, paused: false });

  // Step Configuration Modal State
  const [stepModalOpen, setStepModalOpen] = useState(false);
  const [editingStepId, setEditingStepId] = useState<string | null>(null);
  const [draftStep, setDraftStep] = useState<WorkflowStep>({
    id: '',
    title: 'New Action Step',
    kind: 'click',
    actionType: 'click',
    expectedState: 'server',
    timeoutMs: 5000,
    maxAttempts: 2,
    text: '',
    secretRef: '',
    key: 'ENTER',
  });
  const [stepTestFeedback, setStepTestFeedback] = useState<{ message: string; success: boolean } | null>(null);

  // Account Profiles State (Phase 2)
  const [accountName, setAccountName] = useState('');
  const [accountUsername, setAccountUsername] = useState('');
  const [accountPassword, setAccountPassword] = useState('');
  const [accountTargetName, setAccountTargetName] = useState('');
  const [accountNotes, setAccountNotes] = useState('');
  const [accountFeedback, setAccountFeedback] = useState<{ message: string; success: boolean } | null>(null);
  const [testingVaultTarget, setTestingVaultTarget] = useState<string | null>(null);

  // Sequential Account Queue State (Priority 3)
  const [selectedAccountIds, setSelectedAccountIds] = useState<string[]>([]);
  const [stopOnFailure, setStopOnFailure] = useState(true);
  const [pauseBetweenAccounts, setPauseBetweenAccounts] = useState(false);
  const [accountQueue, setAccountQueue] = useState<AccountQueueItem[]>([]);
  const [currentQueueIndex, setCurrentQueueIndex] = useState<number>(-1);
  const [isSequentialRunning, setIsSequentialRunning] = useState(false);
  const skipAccountRequested = useRef(false);

  // Import / Export State (Priority 4)
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [importedProjectDraft, setImportedProjectDraft] = useState<Project | null>(null);
  const [importSetupAccounts, setImportSetupAccounts] = useState<{ id: string; name: string; username: string; target: string; passwordInput: string; saved: boolean }[]>([]);

  const project = projects.find((item) => item.id === selectedId) ?? null;
  const selectedModels: string[] = connection.phase === 'ready' ? connection.status.models : [];
  const canChat = aiEnabled && connection.phase === 'ready' && model.length > 0 && !busy;

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
  useEffect(() => { if (aiEnabled) void refreshAi(); }, [aiEnabled, refreshAi]);

  // Load windows on mount if in desktop
  useEffect(() => {
    if (isDesktop()) {
      void refreshWindows();
    }
  }, []);

  // Global Emergency Stop shortcut (Escape or Pause key) when automation is running
  useEffect(() => {
    function handleGlobalKeyDown(e: KeyboardEvent) {
      if ((e.key === 'Escape' || e.code === 'Pause') && (runPhase === 'running' || automationBusy)) {
        e.preventDefault();
        void emergencyStop();
      }
    }
    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown);
  }, [runPhase, automationBusy]);

  function toggleAi() {
    if (aiEnabled) {
      setAiEnabled(false);
      setConnection({ phase: 'disabled' });
      setModel('');
      setChatError('');
      return;
    }
    setAiEnabled(true);
  }

  async function refreshWindows() {
    try {
      const windows = await listTargetWindows();
      setTargetWindows(windows);
      setTargetWindowId((current) => {
        if (current && windows.some((w) => w.id === current)) return current;
        // Default to demo if open or first window
        const demo = windows.find((w) => w.isDemo);
        return demo ? demo.id : (windows[0]?.id ?? null);
      });
      setAutomationMessage(windows.length ? `พบ ${windows.length} หน้าต่าง — เลือกหน้าต่างเป้าหมายเพื่อเริ่ม Automation` : 'ไม่พบหน้าต่างที่เลือกได้');
    } catch (error) {
      setAutomationMessage(String(error));
    }
  }

  async function handleBrowseExe() {
    try {
      const path = await pickExecutable();
      if (path && project) {
        updateProjectTarget({ executablePath: path });
        setAutomationMessage(`เลือก Executable: ${path}`);
      }
    } catch (error) {
      setAutomationMessage(`File picker error: ${error}`);
    }
  }

  async function handleLaunchApp() {
    if (!project) return;
    setAutomationBusy(true);
    setAutomationMessage('กำลังเปิดโปรแกรม...');
    try {
      const exe = project.target.executablePath.trim();
      if (!exe || exe.toLowerCase() === 'demo') {
        await startDemoTarget();
        setAutomationMessage('MIMIC Automation Demo เริ่มต้นสำเร็จแล้ว');
      } else {
        const pid = await launchApplication(exe);
        setAutomationMessage(`เปิดแอปสำเร็จ (PID: ${pid})`);
      }
      await new Promise((res) => setTimeout(res, 600));
      await refreshWindows();
    } catch (error) {
      setAutomationMessage(String(error));
    } finally {
      setAutomationBusy(false);
    }
  }

  async function handleSelectWindow(windowId: number) {
    setTargetWindowId(windowId);
    const selected = targetWindows.find((w) => w.id === windowId);
    if (selected && project) {
      const baseTitle = selected.isDemo
        ? 'MIMIC Automation Demo'
        : (selected.title.includes(' - ') ? selected.title.split(' - ')[0].trim() : selected.title);
      updateProjectTarget({
        windowTitleContains: project.target.windowTitleContains || baseTitle,
        executablePath: project.target.executablePath || selected.exePath,
        targetWindowId: windowId,
      });
      try {
        await focusWindow(windowId);
      } catch {
        // ignore
      }
    }
  }

  async function emergencyStop() {
    runControl.current.stopped = true;
    runControl.current.paused = false;
    setRunPhase('stopped');
    try {
      const result = await requestEmergencyStop();
      setAutomationMessage(result.message);
      addRunEvent('system', result.message, 'error');
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

  function openCreateStepModal() {
    setEditingStepId(null);
    setDraftStep({
      id: newId(),
      title: 'New Step',
      kind: 'click',
      actionType: 'click',
      expectedState: 'server',
      timeoutMs: 5000,
      maxAttempts: 2,
      text: '',
      secretRef: '',
      key: 'ENTER',
    });
    setStepTestFeedback(null);
    setStepModalOpen(true);
  }

  function openEditStepModal(step: WorkflowStep) {
    setEditingStepId(step.id);
    setDraftStep({ ...step });
    setStepTestFeedback(null);
    setStepModalOpen(true);
  }

  async function handleStartSelectOnScreen(targetField: 'action' | 'expectedResult' = 'action') {
    setAutomationMessage('เข้าสู่โหมด Select on Screen: เลื่อนเมาส์ไปที่ Element แล้วคลิก หรือลากครอบพื้นที่ภาพ (ESC เพื่อยกเลิก)');
    try {
      const result: SelectOnScreenResult = await selectOnScreen(targetWindowId ?? undefined);
      if (result.cancelled) {
        setAutomationMessage('ยกเลิกการเลือกบนหน้าจอ');
        return;
      }

      const locator: Locator = {
        kind: result.mode === 'region' ? 'template' : 'uia',
        automationId: result.automationId,
        name: result.elementName,
        controlType: result.controlType,
        className: result.className,
        imageBase64: result.imageBase64,
        x: result.x,
        y: result.y,
        width: result.width,
        height: result.height,
      };

      if (targetField === 'expectedResult') {
        // Assign to expected result locator
        setDraftStep((prev) => ({
          ...prev,
          expectedLocator: locator,
        }));
        setAutomationMessage(`บันทึก Expected Result จากหน้าจอ: ${result.elementName || result.controlType || 'Image Region'}`);
      } else {
        // Assign to action target
        const autoTitle = result.elementName
          ? `Click "${result.elementName}"`
          : result.controlType
          ? `Select ${result.controlType}`
          : 'Screen Action';

        const suggestedAction: ActionType =
          result.controlType?.toLowerCase().includes('edit') ? 'type-text' : 'click';

        setDraftStep((prev) => ({
          ...prev,
          id: prev.id || newId(),
          title: autoTitle,
          kind: suggestedAction,
          actionType: suggestedAction,
          locator,
        }));
        setEditingStepId(null);
        setStepTestFeedback(null);
        setStepModalOpen(true);
        setAutomationMessage(`ตรวจพบ Element: ${result.elementName || result.controlType || 'Image Region'} (${result.width}x${result.height}px)`);
      }
    } catch (error) {
      setAutomationMessage(`Select on Screen error: ${error}`);
    }
  }

  function saveDraftStep() {
    if (!project || !draftStep.title.trim()) return;
    const finalStep: WorkflowStep = {
      ...draftStep,
      id: draftStep.id || newId(),
    };

    if (editingStepId) {
      // Update existing
      updateWorkflowStep(editingStepId, finalStep);
    } else {
      // Add new
      setProjects((current) => current.map((item) => item.id !== project.id ? item : {
        ...item,
        workflow: { ...item.workflow, steps: [...item.workflow.steps, finalStep] },
      }));
    }
    setStepModalOpen(false);
    setAutomationMessage(`บันทึก Step "${finalStep.title}" เรียบร้อยแล้ว`);
  }

  async function handleTestSingleStep(stepToTest: WorkflowStep) {
    const isLaunch = stepToTest.actionType === 'launch-app' || stepToTest.kind === 'launch-app';
    if (!targetWindowId && !isLaunch) {
      setStepTestFeedback({ message: 'กรุณาเลือก Target Window ก่อนทดสอบ', success: false });
      return;
    }
    if (!permissionGranted) {
      setStepTestFeedback({
        message: 'กรุณาเปิดสิทธิ์ Permission (ติ๊กถูกช่องอนุญาต Automation ด้านบน) ก่อนทดสอบ Step',
        success: false,
      });
      return;
    }

    const activeAccount = project?.accounts?.find((a) => a.id === project.activeAccountId);
    let resolvedText = stepToTest.text;
    let resolvedSecretRef = stepToTest.secretRef;

    if (activeAccount) {
      if (stepToTest.actionType === 'type-text' || stepToTest.kind === 'type-text') {
        if (!resolvedText || resolvedText === '{username}' || resolvedText.trim() === '') {
          resolvedText = activeAccount.username;
        } else if (resolvedText.includes('{username}')) {
          resolvedText = resolvedText.replace(/\{username\}/g, activeAccount.username);
        }
      } else if (stepToTest.actionType === 'type-secret' || stepToTest.kind === 'type-secret') {
        if (!resolvedSecretRef || resolvedSecretRef.trim() === '') {
          resolvedSecretRef = `vault:${activeAccount.credentialTarget}`;
        }
      }
    }

    const resolvedStep: WorkflowStep = {
      ...stepToTest,
      text: resolvedText,
      secretRef: resolvedSecretRef,
    };

    setAutomationBusy(true);
    setStepTestFeedback(null);
    try {
      await resetEmergencyStop();
      const matchedWin = targetWindows.find((w) => w.id === targetWindowId);
      const scope = {
        targetProcessId: matchedWin?.processId,
        targetExePath: matchedWin?.exePath || project?.target.executablePath,
        targetWindowTitle: project?.target.windowTitleContains?.trim() || undefined,
      };
      // Native Session Security: Authorize target session
      await authorizeSession(targetWindowId ?? 0, scope.targetExePath);

      const result: StepExecutionResult = await executeStep(targetWindowId ?? 0, resolvedStep, permissionGranted, scope);
      if (result.targetWindowId && result.targetWindowId !== targetWindowId) {
        setTargetWindowId(result.targetWindowId);
        void refreshWindows();
      }
      setStepTestFeedback({
        message: result.message,
        success: result.success,
      });
      addRunEvent(stepToTest.id, result.message, result.success ? 'success' : 'error');
    } catch (error) {
      setStepTestFeedback({ message: String(error), success: false });
      addRunEvent(stepToTest.id, String(error), 'error');
    } finally {
      if (!isSequentialRunning) {
        await revokeSession().catch(() => {});
      }
      setAutomationBusy(false);
    }
  }

  function removeWorkflowStep(stepId: string) {
    if (!project || project.workflow.steps.length <= 1) return;
    setProjects((current) => current.map((item) => item.id !== project.id ? item : {
      ...item,
      workflow: { ...item.workflow, steps: item.workflow.steps.filter((step) => step.id !== stepId) },
    }));
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

  function addRunEvent(stepId: string, message: string, status: 'info' | 'success' | 'warning' | 'error' = 'info') {
    setRunEvents((current) => [...current, { stepId, message, at: new Date().toLocaleTimeString(), status }].slice(-40));
  }

  async function waitForResume(): Promise<boolean> {
    while (runControl.current.paused && !runControl.current.stopped) {
      await new Promise((resolve) => window.setTimeout(resolve, 100));
    }
    return !runControl.current.stopped;
  }

  async function runWorkflow(accountOverride?: AccountProfile): Promise<boolean> {
    if (!project || !permissionGranted || (automationBusy && !isSequentialRunning)) return false;
    const firstStepIsLaunch = project.workflow.steps[0]?.actionType === 'launch-app' || project.workflow.steps[0]?.kind === 'launch-app';
    if (!firstStepIsLaunch && targetWindowId === null) {
      setAutomationMessage('กรุณาเลือก Target Window ก่อนสั่ง Run');
      return false;
    }

    runControl.current = { stopped: false, paused: false };
    setAutomationBusy(true);
    setRunPhase('running');
    if (!isSequentialRunning) {
      setRunEvents([]);
    }
    const currentAccount = accountOverride || project.accounts?.find((a) => a.id === project.activeAccountId);
    const accountLabel = currentAccount ? ` [Account: ${currentAccount.name} (${currentAccount.username})]` : '';
    addRunEvent('workflow', `เริ่มต้นรัน Workflow: ${project.workflow.name}${accountLabel} (${project.workflow.steps.length} ขั้นตอน)`, 'info');

    let currentWindowId = targetWindowId ?? 0;

    try {
      await resetEmergencyStop();
      const matchedWin = targetWindows.find((w) => w.id === currentWindowId);
      const exeScope = matchedWin?.exePath || project.target.executablePath;

      // Native Session Security: Authorize session
      await authorizeSession(currentWindowId, exeScope);

      // Ensure target window is in focus if window already exists
      if (currentWindowId > 0) {
        try {
          await focusWindow(currentWindowId);
        } catch {
          // ignore
        }
      }

      for (let i = 0; i < project.workflow.steps.length; i++) {
        if (skipAccountRequested.current) {
          throw new Error('Account skipped by user');
        }
        const step = project.workflow.steps[i]!;
        setRunStepId(step.id);
        if (!await waitForResume()) throw new Error('Workflow หยุดโดยผู้ใช้');

        addRunEvent(step.id, `Step ${i + 1}/${project.workflow.steps.length}: กำลังทำ "${step.title}"...`, 'info');

        let stepSucceeded = false;
        let lastError = '';

        for (let attempt = 1; attempt <= Math.max(1, step.maxAttempts); attempt++) {
          if (skipAccountRequested.current) {
            throw new Error('Account skipped by user');
          }
          if (!await waitForResume()) throw new Error('Workflow หยุดโดยผู้ใช้');
          try {
            let resolvedText = step.text;
            let resolvedSecretRef = step.secretRef;

            if (currentAccount) {
              if (step.actionType === 'type-text' || step.kind === 'type-text') {
                if (!resolvedText || resolvedText === '{username}' || resolvedText.trim() === '') {
                  resolvedText = currentAccount.username;
                } else if (resolvedText.includes('{username}')) {
                  resolvedText = resolvedText.replace(/\{username\}/g, currentAccount.username);
                }
              } else if (step.actionType === 'type-secret' || step.kind === 'type-secret') {
                if (!resolvedSecretRef || resolvedSecretRef.trim() === '') {
                  resolvedSecretRef = `vault:${currentAccount.credentialTarget}`;
                }
              }
            }

            const stepToRun: WorkflowStep = {
              ...step,
              text: (step.actionType === 'launch-app' || step.kind === 'launch-app') && (!step.text || step.text.trim().length === 0)
                ? (project.target.executablePath || 'DEMO')
                : resolvedText,
              secretRef: resolvedSecretRef,
            };

            const curWin = targetWindows.find((w) => w.id === currentWindowId);
            const scope = {
              targetProcessId: curWin?.processId,
              targetExePath: curWin?.exePath || project.target.executablePath,
              targetWindowTitle: project.target.windowTitleContains?.trim() || undefined,
            };

            const result: StepExecutionResult = await executeStep(currentWindowId, stepToRun, permissionGranted, scope);
            if (result.targetWindowId && result.targetWindowId !== currentWindowId) {
              currentWindowId = result.targetWindowId;
              setTargetWindowId(result.targetWindowId);
              void refreshWindows();
            }
            if (result.status === 'stopped') {
              throw new Error(result.message);
            }
            if (!result.success) {
              throw new Error(result.message);
            }
            addRunEvent(step.id, `✓ ${step.title}: ${result.message} (${result.durationMs}ms)`, 'success');
            stepSucceeded = true;
            break;
          } catch (err) {
            lastError = String(err);
            if (attempt < step.maxAttempts) {
              addRunEvent(step.id, `Attempt ${attempt} ล้มเหลว (${lastError}) — ลองใหม่...`, 'warning');
              await new Promise((res) => setTimeout(res, 300));
            }
          }
        }

        if (!stepSucceeded) {
          throw new Error(`Step ${i + 1} (${step.title}) ล้มเหลว: ${lastError}`);
        }
      }

      setRunPhase('completed');
      setAutomationMessage('✓ Workflow ดำเนินการเสร็จสมบูรณ์และตรวจสอบสถานะผ่านทุกขั้นตอน');
      addRunEvent('workflow', 'Workflow completed with every expected state verified.', 'success');
      return true;
    } catch (error) {
      const stopped = runControl.current.stopped;
      setRunPhase(stopped ? 'stopped' : 'failed');
      setAutomationMessage(String(error));
      addRunEvent('workflow', `Workflow halted: ${error}`, 'error');
      return false;
    } finally {
      setRunStepId(null);
      if (!isSequentialRunning) {
        setAutomationBusy(false);
        await revokeSession().catch(() => {});
      }
    }
  }

  // Sequential Workflow Execution (Priority 3)
  async function runSequentialWorkflow() {
    if (!project || !permissionGranted || automationBusy) return;
    const allAccounts = project.accounts || [];
    const accountsToRun = selectedAccountIds.length > 0
      ? allAccounts.filter((a) => selectedAccountIds.includes(a.id))
      : allAccounts;

    if (accountsToRun.length === 0) {
      setAutomationMessage('กรุณาเลือก Account Profile ก่อนสั่งรันแบบ Sequential');
      return;
    }

    setIsSequentialRunning(true);
    setAutomationBusy(true);
    skipAccountRequested.current = false;
    setRunEvents([]);

    const initialQueue: AccountQueueItem[] = accountsToRun.map((account) => ({
      account,
      status: 'pending',
    }));
    setAccountQueue(initialQueue);
    addRunEvent('sequential', `=== เริ่มต้น Sequential Run สำหรับ ${accountsToRun.length} บัญชี ===`, 'info');

    for (let i = 0; i < accountsToRun.length; i++) {
      setCurrentQueueIndex(i);
      const acct = accountsToRun[i]!;

      setAccountQueue((prev) =>
        prev.map((item, idx) => (idx === i ? { ...item, status: 'running' } : item))
      );

      handleSetActiveAccount(acct.id);

      const success = await runWorkflow(acct);

      if (skipAccountRequested.current) {
        setAccountQueue((prev) =>
          prev.map((item, idx) => (idx === i ? { ...item, status: 'skipped', message: 'ผู้ใช้สั่งข้าม' } : item))
        );
        addRunEvent('sequential', `ข้ามบัญชี ${acct.name} (${acct.username}) ตามคำขอ`, 'warning');
        skipAccountRequested.current = false;
        continue;
      }

      if (success) {
        setAccountQueue((prev) =>
          prev.map((item, idx) => (idx === i ? { ...item, status: 'completed', message: 'สำเร็จ' } : item))
        );
        setProjects((current) => current.map((item) => item.id !== project.id ? item : {
          ...item,
          accounts: (item.accounts || []).map((a) => a.id === acct.id ? { ...a, lastUsedAt: new Date().toISOString() } : a),
        }));
      } else {
        setAccountQueue((prev) =>
          prev.map((item, idx) => (idx === i ? { ...item, status: 'failed', message: 'ล้มเหลว' } : item))
        );
        if (stopOnFailure) {
          addRunEvent('sequential', `Sequential Run หยุดทำงาน: บัญชี "${acct.name}" ไม่สำเร็จ (เปิด Stop on failure)`, 'error');
          break;
        }
      }

      if (pauseBetweenAccounts && i < accountsToRun.length - 1) {
        addRunEvent('sequential', 'พักการทำงานชั่วคราวระหว่างบัญชี — ตรวจสอบผลลัพธ์แล้วกด Resume เมื่อพร้อม', 'warning');
        runControl.current.paused = true;
        setRunPhase('paused');
        const resumeSuccess = await waitForResume();
        if (!resumeSuccess) {
          addRunEvent('sequential', 'Sequential Run ถูกยกเลิกโดยผู้ใช้', 'error');
          break;
        }
      }
    }

    await revokeSession().catch(() => {});
    setIsSequentialRunning(false);
    setAutomationBusy(false);
    setCurrentQueueIndex(-1);
    addRunEvent('sequential', '=== สิ้นสุดกระบวนการ Sequential Run ===', 'info');
  }

  function skipCurrentAccount() {
    skipAccountRequested.current = true;
    runControl.current.stopped = true;
    addRunEvent('control', 'สั่งข้ามบัญชีปัจจุบัน (Skip account)', 'warning');
  }

  // Export / Import Handlers (Priority 4)
  function handleExportProject() {
    if (!project) return;
    const jsonStr = exportProjectBundle(project);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${project.name.toLowerCase().replace(/[^a-z0-9_-]/g, '_')}_bundle.mimic.json`;
    document.body.appendChild(anchor);
    anchor.click();
    document.body.removeChild(anchor);
    URL.revokeObjectURL(url);
    setAutomationMessage(`Export โปรเจกต์ "${project.name}" สำเร็จ (ปลอดภัย 100%: ไม่มีรหัสผ่านในไฟล์ JSON)`);
  }

  function handleImportFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (event) => {
      const content = event.target?.result;
      if (typeof content !== 'string') return;
      try {
        const res = importProjectBundle(content);
        setImportedProjectDraft(res.project);
      setImportSetupAccounts(
        (res.project.accounts || []).map((acc) => ({
          id: acc.id,
          name: acc.name,
          username: acc.username,
          target: acc.credentialTarget,
          passwordInput: '',
          saved: false,
        }))
      );
      setImportModalOpen(true);
      } catch (err) {
        alert(String(err));
      }
    };
    reader.readAsText(file);
    e.target.value = '';
  }

  async function handleSaveImportAccountVault(index: number) {
    const item = importSetupAccounts[index];
    if (!item || !item.passwordInput) return;
    try {
      await saveCredential(item.target, item.username, item.passwordInput);
      setImportSetupAccounts((prev) =>
        prev.map((acc, idx) => (idx === index ? { ...acc, saved: true, passwordInput: '' } : acc))
      );
    } catch (err) {
      alert(`บันทึกลง Windows Credential Manager ล้มเหลว: ${err}`);
    }
  }

  function handleFinishProjectImport() {
    if (!importedProjectDraft) return;
    setProjects((current) => [...current, importedProjectDraft]);
    setSelectedId(importedProjectDraft.id);
    setView('workspace');
    setImportModalOpen(false);
    setImportedProjectDraft(null);
    setAutomationMessage(`นำเข้าโปรเจกต์ "${importedProjectDraft.name}" เรียบร้อยแล้ว`);
  }

  function pauseOrResumeWorkflow() {
    if (runPhase === 'running') {
      runControl.current.paused = true;
      setRunPhase('paused');
      addRunEvent('control', 'Workflow ถูกพักไว้ชั่วคราว (Paused)', 'warning');
    } else if (runPhase === 'paused') {
      runControl.current.paused = false;
      setRunPhase('running');
      addRunEvent('control', 'Workflow ดำเนินการต่อ (Resumed)', 'info');
    }
  }

  // Account Management Handlers (Phase 2)
  async function handleAddAccountProfile(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (!project) return;
    const name = accountName.trim();
    const username = accountUsername.trim();
    const password = accountPassword;
    const rawTarget = accountTargetName.trim() || `MIMIC:${project.id.slice(0, 8)}:${username}`;

    if (!name || !username || !password) {
      setAccountFeedback({ message: 'กรุณากรอกชื่อโปรไฟล์, Username และ Password ให้ครบถ้วน', success: false });
      return;
    }

    try {
      // Save directly to Windows Credential Manager
      await saveCredential(rawTarget, username, password);

      const newAccount: AccountProfile = {
        id: newId(),
        name,
        username,
        credentialTarget: rawTarget,
        notes: accountNotes.trim(),
        isActive: true,
        lastUsedAt: undefined,
      };

      const existingAccounts = project.accounts || [];
      const updatedAccounts = [...existingAccounts, newAccount];

      setProjects((current) => current.map((item) => item.id !== project.id ? item : {
        ...item,
        accounts: updatedAccounts,
        activeAccountId: newAccount.id,
      }));

      // Zero out form state immediately
      setAccountName('');
      setAccountUsername('');
      setAccountPassword('');
      setAccountTargetName('');
      setAccountNotes('');
      setAccountFeedback({
        message: `บันทึก Account Profile "${name}" ลงใน Windows Credential Manager สำเร็จ (Vault Target: ${rawTarget})`,
        success: true,
      });
    } catch (err) {
      setAccountFeedback({ message: `เกิดข้อผิดพลาดในการบันทึก Credential: ${err}`, success: false });
    }
  }

  async function handleDeleteAccountProfile(profile: AccountProfile) {
    if (!project) return;
    try {
      await deleteCredential(profile.credentialTarget);
      const updatedAccounts = (project.accounts || []).filter((a) => a.id !== profile.id);
      const newActiveId = project.activeAccountId === profile.id
        ? (updatedAccounts[0]?.id ?? null)
        : project.activeAccountId;

      setProjects((current) => current.map((item) => item.id !== project.id ? item : {
        ...item,
        accounts: updatedAccounts,
        activeAccountId: newActiveId,
      }));

      setAccountFeedback({
        message: `ลบ Account Profile "${profile.name}" และข้อมูลใน Windows Credential Manager เรียบร้อยแล้ว`,
        success: true,
      });
    } catch (err) {
      setAccountFeedback({ message: `ลบ Credential ไม่สำเร็จ: ${err}`, success: false });
    }
  }

  async function handleTestVaultRetrieval(targetName: string) {
    setTestingVaultTarget(targetName);
    try {
      const meta = await checkCredential(targetName);
      setAccountFeedback({
        message: `✓ ตรวจสอบ Windows Credential Manager สำเร็จ: พบบัญชี "${meta.username}" (Target: ${meta.targetName}) [ไม่มี Plaintext Secret ส่งผ่าน IPC]`,
        success: true,
      });
    } catch (err) {
      setAccountFeedback({
        message: `✗ ไม่พบบัญชีใน Windows Credential Manager: ${err}`,
        success: false,
      });
    } finally {
      setTestingVaultTarget(null);
    }
  }

  function handleSetActiveAccount(accountId: string) {
    if (!project) return;
    setProjects((current) => current.map((item) => item.id !== project.id ? item : {
      ...item,
      activeAccountId: accountId,
    }));
  }

  function openProject(nextId: string) {
    setSelectedId(nextId);
    setChatHistory([]);
    setChatError('');
    setView('workspace');
    void refreshWindows();
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
          <p>{aiEnabled ? 'พูดคุยกับโมเดลในเครื่องได้เมื่อเชื่อม LM Studio แล้ว ระบบ Automation ทำงานได้อิสระแม้ปิด AI ครับ' : 'Local AI ถูกปิดสำหรับ MIMIC แล้ว Automation ยังทำงานได้ 100%'}</p>
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
          placeholder={aiEnabled ? (canChat ? 'พิมพ์คุยกับ MIMIC...' : 'เชื่อม LM Studio และโหลดโมเดลเพื่อเริ่มแชต') : 'Local AI ถูกปิดใน MIMIC'}
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
        <SideNavItem icon={Users} label="Account Profiles" active={view === 'accounts'} onClick={() => setView('accounts')} />
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
        <div className="breadcrumbs"><span>MIMIC</span><ChevronRight size={15} /><strong>{view === 'home' ? 'Overview' : view === 'workspace' ? (project?.name ?? 'Projects') : view === 'accounts' ? 'Account Profiles' : view === 'teach' ? 'Teach Mode' : view === 'skills' ? 'Skill Library' : 'Settings'}</strong></div>
        <div className="top-actions"><StatusPill connection={connection} /><button type="button" className={'ai-toggle ' + (aiEnabled ? 'on' : 'off')} onClick={toggleAi}>{aiEnabled ? 'AI On' : 'AI Off'}</button><span className="version-tag">v0.1 · Automation Core</span></div>
      </header>

      <div className="page-content">
        {view === 'home' && <>
          <section className="hero">
            <div className="hero-glow" />
            <div className="hero-copy">
              <span className="eyebrow"><Sparkles size={14} /> YOUR AI COMPANION</span>
              <h1>Meet your own <span>MIMIC.</span></h1>
              <p>ระบบ Automation & ผู้ช่วยส่วนตัว ออกแบบมาให้สอนงานผ่านการชี้และเลือกบนหน้าจอจริง ทำงานแบบ Local-first โดยไม่ต้องพึ่งพา AI หรือ Cloud</p>
              <div className="hero-buttons">
                <button type="button" className="primary-button" onClick={() => setCreating(true)}><Plus size={18} /> สร้าง Project <ArrowRight size={17} /></button>
                <button type="button" className="secondary-button" onClick={() => setView('settings')}><Activity size={17} /> ตรวจสอบ Local AI</button>
                <label className="secondary-button" style={{ display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer', margin: 0 }}>
                  <Upload size={17} /> นำเข้า Project (.json)
                  <input type="file" accept=".json" onChange={handleImportFileChange} style={{ display: 'none' }} />
                </label>
              </div>
            </div>
            <div className="hero-visual" aria-hidden="true">
              <div className="orbit one" /><div className="orbit two" />
              <div className="hero-orb"><Sparkles size={52} strokeWidth={1.4} /></div>
              <span className="orbit-label label-one">observe</span><span className="orbit-label label-two">teach</span><span className="orbit-label label-three">verify</span>
            </div>
          </section>
          <div className="metrics-grid">
            <div className="metric-card"><span><FolderOpen size={20} /></span><div><strong>{projects.length}</strong><small>Projects created</small></div></div>
            <div className="metric-card"><span><Crosshair size={20} /></span><div><strong>Active</strong><small>Select on Screen</small></div></div>
            <div className="metric-card"><span><Monitor size={20} /></span><div><strong>{targetWindows.length}</strong><small>Windows enumerated</small></div></div>
          </div>
          <div className="content-section-heading"><div><span className="section-eyebrow">YOUR WORKSPACE</span><h2>Projects</h2></div><button type="button" className="text-button" onClick={() => setCreating(true)}>New Project <Plus size={17} /></button></div>
          <div className="project-grid">
            {projects.map((item) => <button key={item.id} type="button" className="project-card" onClick={() => openProject(item.id)}>
              <span className="project-icon"><FolderOpen size={24} /></span>
              <strong>{item.name}</strong><p>{item.description || 'พื้นที่สำหรับสอนงานและจัดเก็บ Workflow ของ MIMIC'}</p>
              <span className="project-foot">Open workspace <ArrowRight size={16} /></span>
            </button>)}
            <button type="button" className="project-card create-card" onClick={() => setCreating(true)}>
              <span className="add-round"><Plus size={25} /></span><strong>Create a project</strong><p>สร้างโปรเจกต์สำหรับแอปหรือเกมที่ต้องการ</p>
              <span className="project-foot">Get started <ArrowRight size={16} /></span>
            </button>
          </div>
        </>}

        {view === 'workspace' && (project ? <>
          <div className="workspace-title">
            <div>
              <span className="section-eyebrow">PROJECT WORKSPACE</span>
              <h1>{project.name}</h1>
              <p>{project.description || 'สร้างและทดสอบ Workflow ผ่านการชี้และเลือกบนหน้าจอจริง'}</p>
            </div>
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
              <button type="button" className="teach-button" onClick={() => void handleStartSelectOnScreen('action')} title="ชี้และเลือกปุ่มหรือองค์ประกอบบนหน้าจอจริง">
                <Crosshair size={16} /> Select on Screen
              </button>
              <span className="chip subtle">Local-first · No AI required</span>
            </div>
          </div>

          <div className="workspace-toolbar">
            {(() => {
              const firstStepIsLaunch = project.workflow.steps[0]?.actionType === 'launch-app' || project.workflow.steps[0]?.kind === 'launch-app';
              return (
                <button
                  type="button"
                  className="toolbar-primary"
                  onClick={() => void runWorkflow()}
                  disabled={automationBusy || (!firstStepIsLaunch && targetWindowId === null) || !permissionGranted}
                  title={!permissionGranted ? 'กรุณาติ๊กอนุญาต Permission ก่อนสั่ง Run' : 'รัน Workflow ทั้งหมด'}
                >
                  <Play size={16} /> Run Workflow
                </button>
              );
            })()}
            {project.accounts && project.accounts.length > 0 && (
              <button
                type="button"
                className="secondary-button"
                onClick={() => void runSequentialWorkflow()}
                disabled={automationBusy || isSequentialRunning || !permissionGranted || (!project.workflow.steps[0]?.actionType?.includes('launch') && targetWindowId === null)}
                title="รัน Workflow ตามลำดับ Account Profiles ที่เลือก"
                style={{ background: '#1c2e4a', borderColor: '#3b82f6', color: '#93c5fd' }}
              >
                <ListOrdered size={15} /> Run Sequential {selectedAccountIds.length > 0 ? `(${selectedAccountIds.length})` : `(${project.accounts.length})`}
              </button>
            )}
            <button type="button" className="secondary-button" onClick={handleExportProject} title="ส่งออก Workflow และ Account Metadata สำหรับเครื่องอื่น">
              <Download size={14} /> Export
            </button>
            <label className="secondary-button" style={{ display: 'inline-flex', alignItems: 'center', gap: '5px', cursor: 'pointer', margin: 0 }} title="นำเข้าโปรเจกต์จากไฟล์ JSON">
              <Upload size={14} /> Import
              <input type="file" accept=".json" onChange={handleImportFileChange} style={{ display: 'none' }} />
            </label>
            <button type="button" onClick={pauseOrResumeWorkflow} disabled={runPhase !== 'running' && runPhase !== 'paused'}>
              {runPhase === 'paused' ? <><Play size={15} /> Resume</> : <><Pause size={15} /> Pause</>}
            </button>
            <button type="button" className="emergency-button" onClick={() => void emergencyStop()}>
              <Square size={15} /> Emergency Stop
            </button>
            <button type="button" className="secondary-button" onClick={() => void handleStartSelectOnScreen('action')}>
              <Crosshair size={15} /> + Teach Step
            </button>

            {project.accounts && project.accounts.length > 0 && (
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', background: '#14253d', border: '1px solid #2a4365', borderRadius: '8px', padding: '4px 8px' }}>
                <Users size={14} style={{ color: '#7bbcfc' }} />
                <select
                  value={project.activeAccountId ?? ''}
                  onChange={(e) => handleSetActiveAccount(e.target.value)}
                  style={{ background: 'transparent', border: 0, color: '#e0efff', fontSize: '11px', fontWeight: 600, outline: 'none' }}
                  title="เลือก Account Profile ที่ต้องการใช้กับ Workflow นี้"
                >
                  <option value="" style={{ background: '#0e1b2d' }}>-- ไม่มี Account Profile --</option>
                  {project.accounts.map((acc) => (
                    <option key={acc.id} value={acc.id} style={{ background: '#0e1b2d' }}>
                      {acc.name} ({acc.username})
                    </option>
                  ))}
                </select>
              </div>
            )}
            <button type="button" className="secondary-button" onClick={() => setView('accounts')} title="จัดการ Account Profiles และรหัสผ่าน">
              <KeyRound size={14} /> Accounts
            </button>

            <span className="toolbar-note"><LockKeyhole size={14} /> Scope Enforced · Safe Input</span>
          </div>

          <div className="workspace-grid">
            {/* WORKFLOW BUILDER PANEL */}
            <section className="panel skill-panel workflow-panel">
              <div className="panel-heading">
                <div>
                  <span className="section-eyebrow">WORKFLOW BUILDER</span>
                  <h3><WandSparkles size={18} /> {project.workflow.name}</h3>
                </div>
                <div className="panel-actions">
                  <button className="text-button" type="button" onClick={() => void handleStartSelectOnScreen('action')} title="ชี้และเลือกบนหน้าจอ">
                    <Crosshair size={14} /> Select on Screen
                  </button>
                  <button className="text-button" type="button" onClick={openCreateStepModal}>+ Add step</button>
                </div>
              </div>

              {/* APPLICATION SCOPE SECTION */}
              <div className="application-scope">
                <span className="section-eyebrow">APPLICATION SCOPE</span>
                <div className="scope-row">
                  <label>
                    Target Executable Path
                    <input
                      aria-label="Executable path"
                      value={project.target.executablePath}
                      placeholder="เช่น C:\Games\Yulgang\Client.exe หรือ DEMO"
                      onChange={(event) => updateProjectTarget({ executablePath: event.target.value })}
                    />
                  </label>
                  <button type="button" className="secondary-button" onClick={() => void handleBrowseExe()}>Browse...</button>
                  <button type="button" className="primary-button" onClick={() => void handleLaunchApp()} disabled={automationBusy}>
                    <Play size={14} /> Launch
                  </button>
                </div>

                <div className="scope-row">
                  <label>
                    Target Window (จากหน้าต่างที่เปิดอยู่)
                    <select
                      value={targetWindowId ?? ''}
                      onChange={(event) => handleSelectWindow(Number(event.target.value))}
                    >
                      <option value="">-- เลือกหน้าต่างเป้าหมาย --</option>
                      {targetWindows.map((win) => (
                        <option key={win.id} value={win.id}>
                          {win.isDemo ? '⭐ [DEMO] ' : ''}{win.title} (PID: {win.processId})
                        </option>
                      ))}
                    </select>
                  </label>
                  <button type="button" className="secondary-button" onClick={() => void refreshWindows()} title="รีเฟรชรายการหน้าต่าง">
                    <RefreshCw size={14} /> Detect
                  </button>
                </div>

                <div className="permission-check">
                  <input
                    type="checkbox"
                    id="permCheck"
                    checked={permissionGranted}
                    onChange={(e) => setPermissionGranted(e.target.checked)}
                  />
                  <label htmlFor="permCheck">
                    อนุญาตให้ MIMIC ควบคุมเฉพาะหน้าต่างที่เลือกนี้ (Fail-closed เมื่ออยู่นอก Scope)
                  </label>
                </div>
              </div>

              {/* STEPS LIST */}
              <div className="workflow-steps">
                {project.workflow.steps.map((step, index) => (
                  <div className={'workflow-step ' + (step.id === runStepId ? 'active' : '')} key={step.id}>
                    <strong>{index + 1}</strong>

                    {step.locator?.imageBase64 ? (
                      <img src={step.locator.imageBase64} alt="Target Element" className="step-thumbnail" />
                    ) : (
                      <div className="step-thumbnail" style={{ display: 'grid', placeItems: 'center', color: '#688eb8' }}>
                        <Crosshair size={14} />
                      </div>
                    )}

                    <div className="step-details">
                      <div className="step-title-row">
                        <strong>{step.title}</strong>
                        <span className="chip action">{step.actionType || step.kind}</span>
                        {step.expectedState && <span className="chip verify">verify: {step.expectedState}</span>}
                      </div>
                      <div className="step-meta-row">
                        <span>Target: {step.locator?.name || step.locator?.controlType || (step.locator?.kind === 'template' ? 'Image Region' : 'Window')}</span>
                        <span>· {step.timeoutMs}ms</span>
                        <span>· {step.maxAttempts} retries</span>
                      </div>
                    </div>

                    <div className="step-actions">
                      <button type="button" onClick={() => void handleTestSingleStep(step)} title="ทดสอบเฉพาะ Step นี้">
                        <Play size={13} />
                      </button>
                      <button type="button" onClick={() => openEditStepModal(step)} title="แก้ไข Step">
                        <Settings2 size={13} />
                      </button>
                      <button type="button" onClick={() => moveWorkflowStep(step.id, -1)} disabled={index === 0} title="เลื่อนขึ้น">↑</button>
                      <button type="button" onClick={() => moveWorkflowStep(step.id, 1)} disabled={index === project.workflow.steps.length - 1} title="เลื่อนลง">↓</button>
                      <button type="button" onClick={() => removeWorkflowStep(step.id)} disabled={project.workflow.steps.length <= 1} title="ลบ">
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </section>

            {/* AUTOMATION MONITOR PANEL */}
            <section className="panel screen-panel">
              <div className="panel-heading">
                <div>
                  <span className="section-eyebrow">OBSERVE → ACT → VERIFY</span>
                  <h3><Monitor size={18} /> Automation Monitor</h3>
                </div>
                <span className={'chip ' + (runPhase === 'completed' ? 'verify' : runPhase === 'running' ? 'action' : 'subtle')}>
                  {runPhase}
                </span>
              </div>
              <div className="screen-placeholder">
                {/* SEQUENTIAL RUN QUEUE MONITOR (Priority 3) */}
                {accountQueue.length > 0 && (
                  <div style={{ width: '100%', maxWidth: 520, background: '#122033', border: '1px solid #233b5c', borderRadius: 8, padding: 12, marginBottom: 12 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                      <span style={{ fontSize: 12, fontWeight: 700, color: '#93c5fd', display: 'flex', alignItems: 'center', gap: 6 }}>
                        <ListOrdered size={14} /> Sequential Queue ({accountQueue.filter(q => q.status === 'completed').length}/{accountQueue.length} เสร็จสิ้น)
                      </span>
                      {isSequentialRunning && (
                        <button
                          type="button"
                          className="secondary-button"
                          style={{ padding: '3px 8px', fontSize: 11, background: '#3b2512', color: '#fcd34d', borderColor: '#78350f' }}
                          onClick={skipCurrentAccount}
                          title="ข้ามบัญชีที่กำลังรันอยู่ไปบัญชีถัดไป"
                        >
                          <SkipForward size={13} /> Skip Account
                        </button>
                      )}
                    </div>
                    <div style={{ display: 'grid', gap: 6, maxHeight: 130, overflowY: 'auto' }}>
                      {accountQueue.map((item, idx) => (
                        <div
                          key={item.account.id}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            padding: '4px 8px',
                            borderRadius: 6,
                            background: idx === currentQueueIndex ? '#1e3a5f' : '#0e1724',
                            border: idx === currentQueueIndex ? '1px solid #3b82f6' : '1px solid transparent',
                            fontSize: 11,
                          }}
                        >
                          <span><strong>{idx + 1}. {item.account.name}</strong> ({item.account.username})</span>
                          <span className={'chip ' + (item.status === 'completed' ? 'verify' : item.status === 'running' ? 'action' : item.status === 'failed' ? 'error' : 'subtle')} style={{ fontSize: 9, padding: '2px 6px' }}>
                            {item.status}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                <div className="screen-art">
                  <Monitor size={51} strokeWidth={1.25} />
                  <span className="screen-glint" />
                </div>
                <h3>{runPhase === 'completed' ? 'Workflow Completed Successfully' : 'Ready for Execution'}</h3>
                <p>{automationMessage}</p>
                <div className="demo-flow">
                  <span>Observe Target</span><ArrowRight size={14} />
                  <span>Execute Action</span><ArrowRight size={14} />
                  <span>Verify State</span>
                </div>
                <div className="run-log" aria-live="polite">
                  {runEvents.length ? runEvents.map((event, index) => (
                    <p key={`${event.at}-${index}`} className={event.status === 'success' ? 'log-success' : event.status === 'error' ? 'log-error' : ''}>
                      <time>{event.at}</time> {event.message}
                    </p>
                  )) : <p>ยังไม่มีบันทึก Event การทำงาน (กด Run เพื่อเริ่ม)</p>}
                </div>
              </div>
              <div className="screen-status">
                <span><span className="status-dot" /> {runPhase}{targetWindowId ? ` · Target Window ID: ${targetWindowId}` : ' · No Target Selected'}</span>
                <span><LockKeyhole size={14} /> Safe & Offline Verified</span>
              </div>
            </section>

            {renderChat()}
          </div>
        </> : <EmptyFeature icon={FolderOpen} title="Select a project" description="กลับไปหน้าหลักแล้วสร้างหรือเลือกโปรเจกต์ก่อน" />)}

        {view === 'accounts' && <div className="feature-page" style={{ maxWidth: 960 }}>
          <span className="section-eyebrow">PHASE 2 · SECURITY & PROFILES</span>
          <h1>Account Profiles & Windows Credential Manager</h1>
          <p>
            บริหารจัดการบัญชีผู้ใช้แบบ Dynamic โดยไม่จำกัดจำนวนบัญชี (ไม่มีการ Hardcode บัญชีตายตัว)
            และจัดเก็บรหัสผ่านใน Windows Credential Manager อย่างปลอดภัย ไม่บันทึก Plaintext Secret ลงไฟล์โปรเจกต์
          </p>
          {project && (
            <div style={{ display: 'flex', gap: 16, alignItems: 'center', background: '#122338', border: '1px solid #233e63', borderRadius: 8, padding: '10px 14px', marginTop: 14, flexWrap: 'wrap' }}>
              <span style={{ fontSize: 12, fontWeight: 700, color: '#93c5fd', display: 'flex', alignItems: 'center', gap: 6 }}>
                <ListOrdered size={14} /> Sequential Execution Settings:
              </span>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: '#e0efff', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={stopOnFailure}
                  onChange={(e) => setStopOnFailure(e.target.checked)}
                />
                Stop on Failure (หยุดเมื่อมีบัญชีผิดพลาด)
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: '#e0efff', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={pauseBetweenAccounts}
                  onChange={(e) => setPauseBetweenAccounts(e.target.checked)}
                />
                Pause Between Accounts (พักระหว่างบัญชี)
              </label>
              <button
                type="button"
                className="secondary-button"
                style={{ padding: '3px 8px', fontSize: 11, marginLeft: 'auto' }}
                onClick={() => {
                  if (selectedAccountIds.length === (project.accounts || []).length) {
                    setSelectedAccountIds([]);
                  } else {
                    setSelectedAccountIds((project.accounts || []).map(a => a.id));
                  }
                }}
              >
                {selectedAccountIds.length === (project.accounts || []).length ? 'Deselect All' : 'Select All for Sequential'}
              </button>
            </div>
          )}

          <div style={{ display: 'grid', gridTemplateColumns: 'minmax(320px, 1fr) minmax(360px, 1.2fr)', gap: 20, marginTop: 24 }}>
            {/* ADD ACCOUNT FORM */}
            <div className="panel" style={{ padding: 20 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 16 }}>
                <KeyRound size={20} style={{ color: '#68b6ff' }} />
                <h3 style={{ margin: 0, fontSize: 16 }}>เพิ่ม / บันทึก Account Profile</h3>
              </div>

              {!project ? (
                <p style={{ color: '#fca5a5', fontSize: 12 }}>กรุณาเลือกโปรเจกต์ก่อนเพื่อเพิ่ม Account Profile</p>
              ) : (
                <form onSubmit={handleAddAccountProfile} style={{ display: 'grid', gap: 12 }}>
                  <div className="form-field">
                    <label>Profile Label (ชื่อโปรไฟล์)</label>
                    <input
                      required
                      placeholder="เช่น บัญชีหลัก, ตัวบัฟ 1, Account A"
                      value={accountName}
                      onChange={(e) => setAccountName(e.target.value)}
                    />
                  </div>

                  <div className="form-field">
                    <label>Username (ชื่อผู้ใช้)</label>
                    <input
                      required
                      placeholder="เช่น my_player_01"
                      value={accountUsername}
                      onChange={(e) => {
                        setAccountUsername(e.target.value);
                        if (!accountTargetName || accountTargetName.startsWith('MIMIC:')) {
                          setAccountTargetName(`MIMIC:${project.name.toLowerCase().replace(/[^a-z0-9]/g, '_') || 'app'}:${e.target.value}`);
                        }
                      }}
                    />
                  </div>

                  <div className="form-field">
                    <label>Password (รหัสผ่าน — บันทึกตรงเข้า Windows Credential Manager)</label>
                    <input
                      required
                      type="password"
                      placeholder="••••••••••••"
                      value={accountPassword}
                      onChange={(e) => setAccountPassword(e.target.value)}
                    />
                  </div>

                  <div className="form-field">
                    <label>Vault Target Key (Target Name ใน Windows Credential Manager)</label>
                    <input
                      value={accountTargetName}
                      onChange={(e) => setAccountTargetName(e.target.value)}
                      placeholder={`MIMIC:${project.id.slice(0, 8)}:username`}
                    />
                    <small style={{ color: '#7e9cbf', fontSize: 10 }}>
                      Prefix ด้วย MIMIC: อัตโนมัติ เพื่อแยก Namespace ใน Windows Vault
                    </small>
                  </div>

                  <div className="form-field">
                    <label>Notes (หมายเหตุเพิ่มเติม — ไม่บังคับ)</label>
                    <input
                      placeholder="เช่น เซิร์ฟเวอร์ 2 / ตัวเด็ก"
                      value={accountNotes}
                      onChange={(e) => setAccountNotes(e.target.value)}
                    />
                  </div>

                  <button type="submit" className="primary-button" style={{ marginTop: 8 }}>
                    <ShieldCheck size={16} /> บันทึกเข้า Windows Credential Manager
                  </button>
                </form>
              )}
            </div>

            {/* LIST OF CONFIGURED ACCOUNTS */}
            <div className="panel" style={{ padding: 20 }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <Users size={20} style={{ color: '#7ee0b5' }} />
                  <h3 style={{ margin: 0, fontSize: 16 }}>บัญชีที่กำหนดค่า ({project?.accounts?.length || 0})</h3>
                </div>
                {project && (
                  <span className="chip action">
                    Project: {project.name}
                  </span>
                )}
              </div>

              {accountFeedback && (
                <div className={accountFeedback.success ? 'inline-success' : 'inline-error'} style={{ marginBottom: 14 }}>
                  {accountFeedback.message}
                </div>
              )}

              {(!project?.accounts || project.accounts.length === 0) ? (
                <div style={{ padding: 24, textAlign: 'center', color: '#7a96b5', border: '1px dashed #243a57', borderRadius: 10 }}>
                  <Users size={32} style={{ opacity: 0.5, margin: '0 auto 8px' }} />
                  <p style={{ margin: 0, fontSize: 12 }}>ยังไม่มี Account Profile ในโปรเจกต์นี้</p>
                  <small style={{ color: '#5e7b9b' }}>เพิ่มบัญชีด้านซ้ายเพื่อใช้งาน Type Secret และตัวแปร {'{username}'}</small>
                </div>
              ) : (
                <div style={{ display: 'grid', gap: 10, maxHeight: 420, overflowY: 'auto' }}>
                  {project.accounts.map((acc) => {
                    const isActive = project.activeAccountId === acc.id;
                    return (
                      <div
                        key={acc.id}
                        style={{
                          padding: 14,
                          borderRadius: 10,
                          border: isActive ? '1px solid #4a9bf0' : '1px solid #243a57',
                          background: isActive ? '#14253e' : '#0e1b2d',
                          display: 'flex',
                          flexDirection: 'column',
                          gap: 8,
                        }}
                      >
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            <input
                              type="checkbox"
                              checked={selectedAccountIds.length === 0 || selectedAccountIds.includes(acc.id)}
                              onChange={(e) => {
                                if (e.target.checked) {
                                  setSelectedAccountIds((prev) => [...prev, acc.id]);
                                } else {
                                  setSelectedAccountIds((prev) => prev.filter(id => id !== acc.id));
                                }
                              }}
                              title="เลือกสำหรับ Sequential Run"
                            />
                            <strong>{acc.name}</strong>
                            {isActive && <span className="chip verify">Active Profile</span>}
                          </div>
                          <span style={{ fontSize: 11, color: '#7ebfff', fontFamily: 'monospace' }}>
                            {acc.username}
                          </span>
                        </div>

                        <div style={{ fontSize: 10, color: '#7392b4', display: 'flex', gap: 12, flexWrap: 'wrap' }}>
                          <span>Vault: <code>{acc.credentialTarget}</code></span>
                          {acc.notes && <span>Note: {acc.notes}</span>}
                          {acc.lastUsedAt && <span>Used: {new Date(acc.lastUsedAt).toLocaleTimeString()}</span>}
                        </div>

                        <div style={{ display: 'flex', gap: 8, marginTop: 4 }}>
                          {!isActive && (
                            <button
                              type="button"
                              className="secondary-button"
                              style={{ padding: '6px 10px', fontSize: 11 }}
                              onClick={() => handleSetActiveAccount(acc.id)}
                            >
                              <CheckCircle2 size={13} /> Set as Active
                            </button>
                          )}
                          <button
                            type="button"
                            className="secondary-button"
                            style={{ padding: '6px 10px', fontSize: 11 }}
                            onClick={() => void handleTestVaultRetrieval(acc.credentialTarget)}
                            disabled={testingVaultTarget === acc.credentialTarget}
                          >
                            <ShieldCheck size={13} /> {testingVaultTarget === acc.credentialTarget ? 'กำลังตรวจ...' : 'Test Vault'}
                          </button>
                          <button
                            type="button"
                            className="secondary-button"
                            style={{ padding: '6px 10px', fontSize: 11, color: '#fca5a5', marginLeft: 'auto' }}
                            onClick={() => void handleDeleteAccountProfile(acc)}
                          >
                            <Trash2 size={13} /> Delete
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              <div style={{ marginTop: 20, padding: 12, background: '#112236', borderRadius: 8, border: '1px solid #233b57' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: '#68b6ff', fontSize: 11, fontWeight: 700 }}>
                  <ShieldCheck size={14} /> Local-Only Security Guarantee
                </div>
                <p style={{ margin: '6px 0 0', fontSize: 10.5, color: '#88a6c8', lineHeight: 1.5 }}>
                  รหัสผ่านถูกเข้ารหัสด้วย Windows Data Protection API (DPAPI) ของเครื่องนี้เท่านั้น หาก Export โปรเจกต์ไปให้ผู้อื่น ข้อมูล Secret จะไม่ติดไปด้วย ผู้รับต้องตั้งค่ารหัสผ่านในเครื่องตนเอง
                </p>
              </div>
            </div>
          </div>
        </div>}

        {view === 'teach' && <div className="feature-page">
          <span className="section-eyebrow">TEACH MODE</span>
          <h1>Manual Teach / Select on Screen</h1>
          <p>สอนโปรแกรมโดยการชี้และเลือกองค์ประกอบบนหน้าจอจริง กำหนด Action และ Expected Result ที่ต้องการ</p>
          <div className="settings-panel panel" style={{ marginTop: '20px' }}>
            <div style={{ padding: '20px' }}>
              <h3>วิธีใช้งาน Select on Screen:</h3>
              <ol style={{ lineHeight: '2.2', fontSize: '13px', color: '#bdd3eb' }}>
                <li>เปิดโปรเจกต์และเลือก Target Window หรือเปิด Demo Target</li>
                <li>กดปุ่ม <strong>Select on Screen</strong> ที่แถบด้านบนหรือปุ่ม + Teach Step</li>
                <li>เลื่อนเมาส์ไปยังปุ่มหรือช่องกรอกข้อมูลในหน้าต่างจริง — จะปรากฏกรอบ Highlight สีฟ้า</li>
                <li>คลิกเพื่อเลือกองค์ประกอบ หรือลากกรอบครอบพื้นที่ภาพ</li>
                <li>หน้าต่าง Step Configuration จะเปิดขึ้นเพื่อให้กำหนด Action (Click, Type Text, Type Secret, ฯลฯ)</li>
                <li>กำหนด Expected Result (เงื่อนไขความสำเร็จ เช่น หน้าถัดไปปรากฏ) แล้วกด Save</li>
              </ol>
              <div style={{ marginTop: '16px' }}>
                <button type="button" className="teach-button" onClick={() => void handleStartSelectOnScreen('action')}>
                  <Crosshair size={17} /> ทดลองเข้าสู่โหมด Select on Screen เดี๋ยวนี้
                </button>
              </div>
            </div>
          </div>
        </div>}

        {view === 'skills' && <div className="feature-page">
          <span className="section-eyebrow">FUTURE CAPABILITY</span>
          <h1>Skill Library</h1>
          <p>จัดเก็บ Reusable Skills และโมเดล Workflow สำหรับนำกลับมาใช้ซ้ำ</p>
          <EmptyFeature icon={BookOpen} title="Skill Templates" description="ระบบ Workflow Editor ใน Workspace ปัจจุบันรองรับการบันทึกและรันได้โดยตรง" />
        </div>}

        {view === 'settings' && <div className="settings-page">
          <span className="section-eyebrow">CONFIGURATION</span>
          <h1>Local AI Settings</h1>
          <p>MIMIC v0.1: ทำงานแบบ Offline Automation โดยไม่จำเป็นต้องเปิด LM Studio</p>
          <section className="panel settings-panel">
            <div className="settings-title">
              <div className="settings-icon"><Cpu size={23} /></div>
              <div><h3>LM Studio Local Endpoint</h3><span>http://127.0.0.1:1234/v1</span></div>
              <StatusPill connection={connection} />
            </div>
            <div className="settings-row">
              <div><strong>Connection</strong><p>ตรวจสอบสถานะ Local Server (LM Studio)</p></div>
              <button className="secondary-button" type="button" onClick={() => void refreshAi()} disabled={!aiEnabled || connection.phase === 'checking'}>
                <RefreshCw size={16} /> Check connection
              </button>
            </div>
            {connection.phase === 'error' && <div className="inline-error" role="alert">{connection.reason}</div>}
            <div className="settings-row">
              <div><strong>Chat model</strong><p>เลือกโมเดลสำหรับการสนทนา (ไม่กระทบ Automation)</p></div>
              <select aria-label="AI Model" value={model} disabled={!aiEnabled || !selectedModels.length} onChange={(event) => setModel(event.target.value)}>
                <option value="">{selectedModels.length ? '-- เลือกโมเดล --' : 'ยังไม่มีโมเดล'}</option>
                {selectedModels.map((item) => <option key={item} value={item}>{item}</option>)}
              </select>
            </div>
            <div className="settings-row">
              <div><strong>Automation Engine</strong><p>ระบบตรวจจับหน้าต่าง, Select on Screen และ Verification ทำงานแบบ Native</p></div>
              <span className="chip verify"><ShieldCheck size={14} /> Ready (No AI required)</span>
            </div>
          </section>
        </div>}
      </div>

      <footer className="app-footer">
        <span><ShieldCheck size={14} /> MIMIC · Windows 11 Local-First Automation</span>
        <span>Deterministic observe → act → verify engine</span>
        <span><CircleHelp size={14} /> Phase 1 Delivered</span>
      </footer>
    </main>

    {/* STEP CONFIGURATION MODAL */}
    {stepModalOpen && (
      <div className="modal-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) setStepModalOpen(false); }}>
        <div className="step-config-modal">
          <div className="modal-top">
            <div className="modal-icon teach"><Crosshair size={24} /></div>
            <button type="button" className="icon-button" onClick={() => setStepModalOpen(false)} aria-label="ปิด"><X size={18} /></button>
          </div>
          <span className="section-eyebrow">STEP CONFIGURATION</span>
          <h2>{editingStepId ? 'แก้ไข Workflow Step' : 'ตั้งค่า Action & Expected Result'}</h2>
          <p>กำหนดการกระทำต่อองค์ประกอบที่เลือก และระบุเงื่อนไขตรวจสอบว่าทำแล้วสำเร็จอย่างไร</p>

          {/* LOCATOR PREVIEW */}
          {draftStep.locator && (
            <div className="preview-card">
              {draftStep.locator.imageBase64 ? (
                <img src={draftStep.locator.imageBase64} alt="Target Element" />
              ) : (
                <div style={{ width: 90, height: 54, background: '#0a1626', display: 'grid', placeItems: 'center', color: '#688eb8', borderRadius: 8 }}>
                  <Crosshair size={22} />
                </div>
              )}
              <div className="preview-info">
                <strong>{draftStep.locator.name || draftStep.locator.controlType || 'Selected Element'}</strong>
                <span>Method: {draftStep.locator.kind === 'uia' ? 'Windows UI Automation' : 'Image Template Matching'}</span>
                {draftStep.locator.automationId && <span>Automation ID: #{draftStep.locator.automationId}</span>}
                {draftStep.locator.width && draftStep.locator.height && <span>Area: {draftStep.locator.width} × {draftStep.locator.height} px</span>}
              </div>
              <button type="button" className="secondary-button" onClick={() => void handleStartSelectOnScreen('action')} style={{ height: 32, fontSize: 11 }}>
                <Crosshair size={13} /> Re-select
              </button>
            </div>
          )}

          <div className="form-grid">
            <div className="form-field">
              <label>Step Title</label>
              <input
                value={draftStep.title}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, title: e.target.value }))}
                placeholder="เช่น Click Login Button หรือ Enter Username"
              />
            </div>

            <div className="form-field">
              <label>Action</label>
              <select
                value={draftStep.actionType || draftStep.kind}
                onChange={(e) => {
                  const val = e.target.value as ActionType;
                  setDraftStep((prev) => ({ ...prev, kind: val, actionType: val }));
                }}
              >
                <option value="click">Click — คลิกองค์ประกอบ</option>
                <option value="double-click">Double Click — ดับเบิลคลิก</option>
                <option value="type-text">Type Text — พิมพ์ข้อความ</option>
                <option value="type-secret">Type Secret — พิมพ์รหัสผ่านจาก Windows Credential Manager</option>
                <option value="press-key">Press Key — กดปุ่มคีย์บอร์ด</option>
                <option value="wait-visible">Wait Until Visible — รอจนพบองค์ประกอบ</option>
                <option value="wait-hidden">Wait Until Hidden — รอจนองค์ประกอบหายไป</option>
                <option value="verify-state">Verify State — ตรวจสอบสถานะ</option>
                <option value="launch-app">Launch Application — เปิดโปรแกรม</option>
              </select>
            </div>
          </div>

          {/* Action-specific Parameters */}
          {(draftStep.actionType === 'launch-app' || draftStep.kind === 'launch-app') && (
            <div className="form-field" style={{ marginTop: 12 }}>
              <label>Executable Path (หรือระบุ "DEMO" สำหรับ Built-in Demo Test Window)</label>
              <input
                value={draftStep.text || ''}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, text: e.target.value }))}
                placeholder="เช่น C:\Program Files\App\app.exe หรือ DEMO"
              />
            </div>
          )}

          {(draftStep.actionType === 'type-text' || draftStep.kind === 'type-text') && (
            <div className="form-field" style={{ marginTop: 12 }}>
              <label>Text to Type (ข้อความทั่วไป ไม่ใช่รหัสผ่าน)</label>
              <input
                value={draftStep.text || ''}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, text: e.target.value }))}
                placeholder="เช่น my_username หรือข้อความที่ต้องการกรอก"
              />
            </div>
          )}

          {(draftStep.actionType === 'type-secret' || draftStep.kind === 'type-secret') && (
            <div className="form-field" style={{ marginTop: 12, padding: 14, background: '#10223b', border: '1px solid #284c78', borderRadius: 8 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: '#7ec1ff', fontWeight: 600, fontSize: 12, marginBottom: 8 }}>
                <KeyRound size={16} />
                <span>Windows Credential Manager Integration (Safe & Secret)</span>
              </div>
              <label>Secret Vault Reference (Target Name ใน Windows Credential Manager)</label>
              {project?.accounts && project.accounts.length > 0 && (
                <div style={{ marginBottom: 8 }}>
                  <select
                    value={draftStep.secretRef || ''}
                    onChange={(e) => setDraftStep((prev) => ({ ...prev, secretRef: e.target.value }))}
                  >
                    <option value="">-- ปล่อยว่างเพื่อใช้ Active Account อัตโนมัติ --</option>
                    {project.accounts.map((acc) => (
                      <option key={acc.id} value={`vault:${acc.credentialTarget}`}>
                        {acc.name} ({acc.username}) → vault:{acc.credentialTarget}
                      </option>
                    ))}
                  </select>
                </div>
              )}
              <input
                value={draftStep.secretRef || ''}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, secretRef: e.target.value }))}
                placeholder="เช่น vault:MIMIC:yulgang:account_1 หรือปล่อยว่างเพื่อใช้ Active Account"
              />
              <p style={{ margin: '6px 0 0', fontSize: 11, color: '#88a8cb', lineHeight: 1.4 }}>
                ความปลอดภัย: รหัสผ่านจะถูกดึงจาก Windows Credential Manager ตรงเข้าหน่วยความจำชั่วคราวและล้างทิ้ง (Zeroize) ทันทีหลังพิมพ์เสร็จ จะไม่มีการบันทึก Plaintext ลงในไฟล์โปรเจกต์
              </p>
            </div>
          )}

          {(draftStep.actionType === 'press-key' || draftStep.kind === 'press-key') && (
            <div className="form-field" style={{ marginTop: 12 }}>
              <label>Keyboard Key</label>
              <select
                value={draftStep.key || 'ENTER'}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, key: e.target.value }))}
              >
                <option value="ENTER">Enter (Return)</option>
                <option value="TAB">Tab</option>
                <option value="ESC">Escape (ESC)</option>
                <option value="SPACE">Spacebar</option>
                <option value="BACKSPACE">Backspace</option>
              </select>
            </div>
          )}

          {/* EXPECTED RESULT CONFIGURATION */}
          <div style={{ marginTop: 18, borderTop: '1px solid #233a57', paddingTop: 14 }}>
            <span className="section-eyebrow">EXPECTED RESULT (POSTCONDITION)</span>
            <p style={{ margin: '4px 0 10px', fontSize: 11, color: '#8aa6c7' }}>
              ทำ Action นี้แล้ว จะรู้ได้อย่างไรว่าสำเร็จ? (เช่น หน้าถัดไปปรากฏ หรือปุ่มเปลี่ยนสถานะ)
            </p>

            <div className="form-grid">
              <div className="form-field">
                <label>Expected State / Window Title</label>
                <select
                  value={draftStep.expectedState || 'server'}
                  onChange={(e) => setDraftStep((prev) => ({ ...prev, expectedState: e.target.value }))}
                >
                  <option value="server">Server Selection Screen</option>
                  <option value="ready">Game / Ready Screen</option>
                  <option value="login">Login Screen</option>
                </select>
              </div>

              <div className="form-field">
                <label>หรือตรวจจาก Element บนหน้าจอ</label>
                <button
                  type="button"
                  className="secondary-button"
                  onClick={() => void handleStartSelectOnScreen('expectedResult')}
                  style={{ height: 38 }}
                >
                  <Crosshair size={14} /> {draftStep.expectedLocator ? 'Expected Element เลือกแล้ว' : 'Select on Screen'}
                </button>
              </div>
            </div>

            {draftStep.expectedLocator && (
              <div className="preview-card" style={{ marginTop: 8 }}>
                {draftStep.expectedLocator.imageBase64 && (
                  <img src={draftStep.expectedLocator.imageBase64} alt="Expected Result" style={{ width: 60, height: 36 }} />
                )}
                <div className="preview-info">
                  <strong>Expected: {draftStep.expectedLocator.name || 'Visual Element'}</strong>
                  <span>{draftStep.expectedLocator.width} × {draftStep.expectedLocator.height} px</span>
                </div>
              </div>
            )}
          </div>

          <div className="form-grid" style={{ marginTop: 14 }}>
            <div className="form-field">
              <label>Timeout (มิลลิวินาที)</label>
              <input
                type="number"
                min="1000"
                max="60000"
                step="500"
                value={draftStep.timeoutMs}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, timeoutMs: Math.max(1000, Number(e.target.value) || 1000) }))}
              />
            </div>
            <div className="form-field">
              <label>Retries (จำนวนครั้งที่ลองใหม่เมื่อไม่พบ)</label>
              <input
                type="number"
                min="1"
                max="5"
                value={draftStep.maxAttempts}
                onChange={(e) => setDraftStep((prev) => ({ ...prev, maxAttempts: Math.max(1, Math.min(5, Number(e.target.value) || 1)) }))}
              />
            </div>
          </div>

          {stepTestFeedback && (
            <div className={stepTestFeedback.success ? 'inline-success' : 'inline-error'} style={{ marginTop: 12 }}>
              {stepTestFeedback.success ? '✓ ' : '✗ '}{stepTestFeedback.message}
            </div>
          )}

          <div className="modal-actions">
            <button
              type="button"
              className="secondary-button"
              onClick={() => void handleTestSingleStep(draftStep)}
              disabled={automationBusy}
            >
              <Play size={14} /> Test Step Now
            </button>
            <div className="modal-actions-right">
              <button type="button" className="secondary-button" onClick={() => setStepModalOpen(false)}>Cancel</button>
              <button type="button" className="primary-button" onClick={saveDraftStep}>
                <Check size={16} /> Save Step
              </button>
            </div>
          </div>
        </div>
      </div>
    )}

    {/* CREATE PROJECT MODAL */}
    {creating && <div className="modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) setCreating(false); }}>
      <form className="create-modal" onSubmit={createProject}>
        <div className="modal-top"><div className="modal-icon"><FolderOpen size={23} /></div><button type="button" className="icon-button" onClick={() => setCreating(false)} aria-label="ปิด"><X size={18} /></button></div>
        <span className="section-eyebrow">NEW WORKSPACE</span><h2>Create a Project</h2>
        <p>แยกพื้นที่และบริบทของแต่ละงาน พร้อมระบบ Automation โดยไม่ใช้ Plaintext Secret</p>
        <label>ชื่อโปรเจกต์<input autoFocus required maxLength={60} placeholder="เช่น โยวกัง หรือ Daily Automation" value={projectName} onChange={(event) => setProjectName(event.target.value)} /></label>
        <label>คำอธิบาย (ไม่บังคับ)<textarea rows={3} maxLength={200} placeholder="โปรเจกต์นี้ใช้สำหรับอะไร?" value={projectDescription} onChange={(event) => setProjectDescription(event.target.value)} /></label>
        <div className="modal-actions"><button type="button" className="secondary-button" onClick={() => setCreating(false)}>Cancel</button><button type="submit" className="primary-button" disabled={!projectName.trim()}><Check size={17} /> Create Project</button></div>
      </form>
    </div>}
  
    {/* IMPORT PROJECT SETUP MODAL (Priority 4) */}
    {importModalOpen && importedProjectDraft && (
      <div className="modal-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) setImportModalOpen(false); }}>
        <div className="create-modal" style={{ maxWidth: 540 }}>
          <div className="modal-top">
            <div className="modal-icon"><Upload size={22} /></div>
            <button type="button" className="icon-button" onClick={() => setImportModalOpen(false)} aria-label="ปิด"><X size={18} /></button>
          </div>
          <span className="section-eyebrow">PORTABILITY & SETUP</span>
          <h2>Setup Imported Project</h2>
          <p>
            นำเข้าโปรเจกต์ <strong>{importedProjectDraft.name}</strong> ตรวจสอบ Executable Path บนเครื่องนี้ และบันทึกรหัสผ่านลง Windows Credential Manager
          </p>

          <div className="form-field" style={{ marginTop: 12 }}>
            <label>Target Executable Path บนเครื่องนี้</label>
            <div style={{ display: 'flex', gap: 8 }}>
              <input
                value={importedProjectDraft.target.executablePath || ''}
                onChange={(e) => setImportedProjectDraft({
                  ...importedProjectDraft,
                  target: { ...importedProjectDraft.target, executablePath: e.target.value }
                })}
                placeholder="C:\Games\Yulgang\Client.exe หรือ DEMO"
                style={{ flex: 1 }}
              />
              <button
                type="button"
                className="secondary-button"
                onClick={async () => {
                  const picked = await pickExecutable();
                  if (picked) {
                    setImportedProjectDraft({
                      ...importedProjectDraft,
                      target: { ...importedProjectDraft.target, executablePath: picked }
                    });
                  }
                }}
              >
                Browse EXE
              </button>
            </div>
          </div>

          <div style={{ marginTop: 14 }}>
            <strong style={{ fontSize: 13, color: '#93c5fd' }}>Account Credentials Setup:</strong>
            <p style={{ fontSize: 11, color: '#7e9cbf', margin: '4px 0 10px' }}>
              ไฟล์ JSON นำเข้าไม่มี Plaintext Password กรุณาตั้งรหัสผ่านสำหรับแต่ละบัญชีเพื่อบันทึกลง Windows Vault บนเครื่องนี้
            </p>
            {importSetupAccounts.length === 0 ? (
              <p style={{ fontSize: 12, color: '#94a3b8' }}>ไม่มีบัญชีที่ต้องตั้งค่าในโปรเจกต์นี้</p>
            ) : (
              <div style={{ display: 'grid', gap: 10, maxHeight: 200, overflowY: 'auto' }}>
                {importSetupAccounts.map((acc, idx) => (
                  <div key={acc.id} style={{ background: '#0e1724', border: '1px solid #1f334d', borderRadius: 6, padding: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                      <span style={{ fontSize: 12, fontWeight: 600 }}>{acc.name} ({acc.username})</span>
                      {acc.saved && <span className="chip verify" style={{ fontSize: 10 }}>Saved to Vault</span>}
                    </div>
                    {!acc.saved ? (
                      <div style={{ display: 'flex', gap: 6, marginTop: 4 }}>
                        <input
                          type="password"
                          placeholder="กรอกรหัสผ่านสำหรับเครื่องนี้"
                          value={acc.passwordInput}
                          onChange={(e) => {
                            const val = e.target.value;
                            setImportSetupAccounts((prev) => prev.map((item, i) => i === idx ? { ...item, passwordInput: val } : item));
                          }}
                          style={{ flex: 1, padding: '4px 8px', fontSize: 12 }}
                        />
                        <button
                          type="button"
                          className="secondary-button"
                          style={{ padding: '4px 8px', fontSize: 11 }}
                          disabled={!acc.passwordInput}
                          onClick={() => void handleSaveImportAccountVault(idx)}
                        >
                          Save
                        </button>
                      </div>
                    ) : (
                      <span style={{ fontSize: 11, color: '#86efac' }}>✓ พร้อมใช้งานบนเครื่องนี้แล้ว</span>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="modal-actions" style={{ marginTop: 18 }}>
            <button type="button" className="secondary-button" onClick={() => setImportModalOpen(false)}>Cancel</button>
            <button type="button" className="primary-button" onClick={handleFinishProjectImport}>
              <Check size={16} /> Finish & Open Project
            </button>
          </div>
        </div>
      </div>
    )}
  </div>;
}
