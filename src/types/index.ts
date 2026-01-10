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

// App state
export type AppState = 'idle' | 'planning' | 'confirm' | 'executing' | 'done' | 'error'

// Store interface
export interface OttoStore {
  state: AppState
  command: string
  plan: ActionPlan | null
  currentStepIndex: number
  error: string | null
  debugLogs: Record<number, string>

  setCommand: (cmd: string) => void
  setPlan: (plan: ActionPlan) => void
  setStepIndex: (idx: number) => void
  setDebugLog: (idx: number, info: string) => void
  setState: (state: AppState) => void
  setError: (err: string) => void
  reset: () => void
}
