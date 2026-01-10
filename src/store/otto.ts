import { create } from 'zustand'
import type { OttoStore, AppState, ActionPlan } from '../types'

export const useOttoStore = create<OttoStore>((set) => ({
  state: 'idle',
  command: '',
  plan: null,
  currentStepIndex: 0,
  error: null,
  debugLogs: {},

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
    }),
}))
