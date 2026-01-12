import { create } from 'zustand'
import type { OttoStore, AppState, ActionPlan, AgentSession, Goal, GoalPipelineState, DecompositionInfo } from '../types'

export const useOttoStore = create<OttoStore>((set) => ({
  // Legacy state
  state: 'idle',
  command: '',
  plan: null,
  currentStepIndex: 0,
  error: null,
  debugLogs: {},

  // New agent state
  agentSession: null,
  useAgentMode: true,
  goalPipelineStates: {},
  decompositionInfo: null,

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
}))
