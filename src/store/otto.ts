import { create } from 'zustand'
import type { OttoStore, AppState, ActionPlan, AgentSession, AgentSessionV2, Goal, GoalPipelineState, DecompositionInfo, LlmDebugEvent, LlmResponseEvent, LlmCallEntry } from '../types'

export const useOttoStore = create<OttoStore>((set) => ({
  // Legacy state
  state: 'idle',
  command: '',
  plan: null,
  currentStepIndex: 0,
  error: null,
  debugLogs: {},

  // New agent state (v1 - goal-based)
  agentSession: null,
  useAgentMode: true,
  goalPipelineStates: {},
  decompositionInfo: null,

  // New agent state (v2 - tool-based)
  agentSessionV2: null,
  useAgentV2: true,  // Use v2 by default

  // Debug state
  llmCalls: {},
  selectedLlmCall: null,

  // Legacy actions
  setCommand: (cmd: string) => set({ command: cmd }),

  setPlan: (plan: ActionPlan) => set({ plan }),

  setStepIndex: (idx: number) => set({ currentStepIndex: idx }),

  setDebugLog: (idx: number, info: string) => set((state) => ({
    debugLogs: { ...state.debugLogs, [idx]: info }
  })),

  setState: (state: AppState) => set({ state }),

  setError: (err: string) => set({ error: err, state: 'error' }),

  reset: () =>
    set({
      state: 'idle',
      command: '',
      plan: null,
      currentStepIndex: 0,
      error: null,
      debugLogs: {},
      agentSession: null,
      goalPipelineStates: {},
      decompositionInfo: null,
      agentSessionV2: null,
      llmCalls: {},
      selectedLlmCall: null,
    }),

  // New agent actions
  setAgentSession: (session: AgentSession) => set({ agentSession: session }),

  updateGoal: (goalId: string, updates: Partial<Goal>) => set((state) => ({
    agentSession: state.agentSession ? {
      ...state.agentSession,
      goals: state.agentSession.goals.map(g =>
        g.id === goalId ? { ...g, ...updates } : g
      )
    } : null
  })),

  setUseAgentMode: (use: boolean) => set({ useAgentMode: use }),

  updateGoalPipeline: (goalId: string, updates: Partial<GoalPipelineState>) => set((state) => ({
    goalPipelineStates: {
      ...state.goalPipelineStates,
      [goalId]: {
        ...state.goalPipelineStates[goalId],
        ...updates
      }
    }
  })),

  setDecompositionInfo: (info: DecompositionInfo) => set({ decompositionInfo: info }),

  // New agent actions (v2)
  setAgentSessionV2: (session: AgentSessionV2) => set({ agentSessionV2: session }),
  setUseAgentV2: (use: boolean) => set({ useAgentV2: use }),

  // Debug actions
  addLlmPrompt: (event: LlmDebugEvent) => set((state) => {
    const entry: LlmCallEntry = {
      id: event.call_id,
      type: event.call_type,
      model: event.model,
      prompt: event.prompt,
      timestamp: event.timestamp,
      status: 'pending'
    }
    return {
      llmCalls: { ...state.llmCalls, [event.call_id]: entry }
    }
  }),

  addLlmResponse: (event: LlmResponseEvent) => set((state) => {
    const existing = state.llmCalls[event.call_id]
    if (!existing) return state

    const updated: LlmCallEntry = {
      ...existing,
      raw_response: event.raw_response,
      parsed_result: event.parsed_result,
      duration_ms: event.duration_ms,
      success: event.success,
      error: event.error,
      status: event.success ? 'success' : 'error'
    }
    return {
      llmCalls: { ...state.llmCalls, [event.call_id]: updated }
    }
  }),

  selectLlmCall: (callId: string | null) => set({ selectedLlmCall: callId }),

  clearLlmCalls: () => set({ llmCalls: {}, selectedLlmCall: null }),
}))
