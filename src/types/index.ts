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

  // New agent state
  agentSession: AgentSession | null
  useAgentMode: boolean
  goalPipelineStates: Record<string, GoalPipelineState>  // goalId -> pipeline state
  decompositionInfo: DecompositionInfo | null

  // Legacy actions
  setCommand: (cmd: string) => void
  setPlan: (plan: ActionPlan) => void
  setStepIndex: (idx: number) => void
  setDebugLog: (idx: number, info: string) => void
  setState: (state: AppState) => void
  setError: (err: string) => void
  reset: () => void

  // New agent actions
  setAgentSession: (session: AgentSession) => void
  updateGoal: (goalId: string, updates: Partial<Goal>) => void
  setUseAgentMode: (use: boolean) => void
  updateGoalPipeline: (goalId: string, updates: Partial<GoalPipelineState>) => void
  setDecompositionInfo: (info: DecompositionInfo) => void
}
