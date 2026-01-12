// Action types
export type ActionType =
  | 'open_app'
  | 'type_text'
  | 'press_key'
  | 'mouse_click'
  | 'mouse_move'
  | 'wait'
  | 'find_and_click'

// Type-specific parameters
export type ActionParams =
  | { app_name: string }                              // open_app
  | { text: string }                                  // type_text
  | { key: string; modifiers?: string[] }             // press_key
  | { x: number; y: number; button?: 'left' | 'right' }  // mouse_click
  | { x: number; y: number }                          // mouse_move
  | { ms: number }                                    // wait
  | { element: string }                               // find_and_click

// Individual action step
export interface ActionStep {
  id: string
  type: ActionType
  description: string
  params: ActionParams
}

// Full execution plan
export interface ActionPlan {
  id: string
  original_command: string
  steps: ActionStep[]
  requires_confirmation: boolean
}

// App state (legacy)
export type AppState = 'idle' | 'planning' | 'confirm' | 'executing' | 'done' | 'error'

// ============================================
// Agent Types (New Reactive Architecture)
// ============================================

export type AgentState =
  | 'idle'
  | 'decomposing'
  | 'observing'
  | 'thinking'
  | 'acting'
  | 'verifying'
  | 'complete'
  | 'error'

export type GoalStatus = 'pending' | 'in_progress' | 'completed' | 'failed'

// How a command was decomposed
export interface DecompositionInfo {
  method: 'pattern' | 'llm'
  pattern_name?: string
  original_command: string
}

export interface Goal {
  id: string
  description: string
  success_criteria: string
  status: GoalStatus
  attempts: number
  max_attempts: number
}

// Pipeline step details for UI display
export interface GoalPipelineState {
  step: 'observing' | 'thinking' | 'acting' | 'verifying' | 'done'
  // Observe step
  observation?: string
  // Think step - what action was decided
  actionType?: string
  actionParams?: string
  actionRationale?: string
  // Act step - execution result
  actionResult?: 'success' | 'failed'
  actionError?: string
  // Verify step
  verification?: string
  verified?: boolean
}

export interface ScreenState {
  timestamp: number
  description: string
  detected_elements: DetectedElement[]
  active_app?: string
  screenshot_hash: string
}

export interface DetectedElement {
  description: string
  location?: [number, number]
  confidence: number
}

export interface AtomicAction {
  id: string
  action_type: ActionType
  params: ActionParams
  rationale: string
}

export interface ActionResult {
  action_id: string
  success: boolean
  error_message?: string
  screen_changed: boolean
}

export interface VerificationResult {
  goal_id: string
  action_id: string
  goal_achieved: boolean
  progress_made: boolean
  observation: string
}

export interface AgentSession {
  id: string
  original_command: string
  goals: Goal[]
  current_goal_index: number
  state: AgentState
  action_history: ActionResult[]
  total_actions: number
  max_total_actions: number
  current_action?: AtomicAction
  last_observation?: ScreenState
  error?: string
}

// ============================================
// LLM Debug Types
// ============================================

export type LlmCallType =
  | 'decomposition'
  | 'screen_description'
  | 'action_decision'
  | 'verification'
  | 'find_element'

export interface LlmDebugEvent {
  call_id: string
  call_type: LlmCallType
  model: string
  prompt: string
  timestamp: number
}

export interface LlmResponseEvent {
  call_id: string
  raw_response: string
  parsed_result?: string
  duration_ms: number
  success: boolean
  error?: string
}

export interface LlmCallEntry {
  id: string
  type: LlmCallType
  model: string
  prompt: string
  timestamp: number
  // Response fields (filled when response arrives)
  raw_response?: string
  parsed_result?: string
  duration_ms?: number
  success?: boolean
  error?: string
  status: 'pending' | 'success' | 'error'
}

// ============================================
// New Tool-based Agent Types (v2)
// ============================================

export type AgentStateV2 = 'idle' | 'planning' | 'executing' | 'done' | 'failed'

export type StepStatus = 'pending' | 'in_progress' | 'done' | 'failed'

export interface PlanStep {
  id: number
  description: string
  status: StepStatus
}

export interface Plan {
  task: string
  steps: PlanStep[]
  current_step: number
}

export interface UIElement {
  label: string
  element_type: string
  x: number
  y: number
}

export interface ToolOutput {
  type: 'screenshot' | 'ack'
  elements?: UIElement[]
  active_app?: string
}

export interface ToolResult {
  tool: string
  success: boolean
  output?: ToolOutput
  error?: string
}

export interface AgentSessionV2 {
  id: string
  task: string
  state: AgentStateV2
  plan: Plan | null
  step_count: number
  error?: string
}

// ============================================
// Store Interface
// ============================================

export interface OttoStore {
  // Legacy support
  state: AppState
  command: string
  plan: ActionPlan | null
  currentStepIndex: number
  error: string | null
  debugLogs: Record<number, string>

  // New agent state (v1 - goal-based)
  agentSession: AgentSession | null
  useAgentMode: boolean
  goalPipelineStates: Record<string, GoalPipelineState>  // goalId -> pipeline state
  decompositionInfo: DecompositionInfo | null

  // New agent state (v2 - tool-based)
  agentSessionV2: AgentSessionV2 | null
  useAgentV2: boolean

  // Debug state
  llmCalls: Record<string, LlmCallEntry>  // call_id -> entry
  selectedLlmCall: string | null

  // Legacy actions
  setCommand: (cmd: string) => void
  setPlan: (plan: ActionPlan) => void
  setStepIndex: (idx: number) => void
  setDebugLog: (idx: number, info: string) => void
  setState: (state: AppState) => void
  setError: (err: string) => void
  reset: () => void

  // New agent actions (v1)
  setAgentSession: (session: AgentSession) => void
  updateGoal: (goalId: string, updates: Partial<Goal>) => void
  setUseAgentMode: (use: boolean) => void
  updateGoalPipeline: (goalId: string, updates: Partial<GoalPipelineState>) => void
  setDecompositionInfo: (info: DecompositionInfo) => void

  // New agent actions (v2)
  setAgentSessionV2: (session: AgentSessionV2) => void
  setUseAgentV2: (use: boolean) => void

  // Debug actions
  addLlmPrompt: (event: LlmDebugEvent) => void
  addLlmResponse: (event: LlmResponseEvent) => void
  selectLlmCall: (callId: string | null) => void
  clearLlmCalls: () => void
}
