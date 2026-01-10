import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useOttoStore } from '../store/otto'
import type { ActionPlan } from '../types'

export function useTauriEvents() {
  const { setPlan, setState, setStepIndex, setError, setDebugLog } = useOttoStore()

  useEffect(() => {
    const unlisteners: (() => void)[] = []

    const setupListeners = async () => {
      // Plan ready event - auto execute immediately
      const unlistenPlan = await listen<ActionPlan>('plan_ready', async (event) => {
        setPlan(event.payload)
        setState('executing')
        // Auto-execute the plan
        try {
          await invoke('execute_plan', { plan: event.payload })
        } catch (err) {
          setError(String(err))
        }
      })
      unlisteners.push(unlistenPlan)

      // Step started event
      const unlistenStepStarted = await listen<{ stepIndex: number; debug: string }>(
        'step_started',
        (event) => {
          setStepIndex(event.payload.stepIndex)
          setDebugLog(event.payload.stepIndex, event.payload.debug)
        }
      )
      unlisteners.push(unlistenStepStarted)

      // Step completed event
      const unlistenStepCompleted = await listen<{
        stepIndex: number
        success: boolean
      }>('step_completed', (event) => {
        if (!event.payload.success) {
          setError(`Step ${event.payload.stepIndex + 1} failed`)
        }
      })
      unlisteners.push(unlistenStepCompleted)

      // Execution done event
      const unlistenDone = await listen<{ success: boolean; message?: string }>(
        'execution_done',
        (event) => {
          if (event.payload.success) {
            setState('done')
          } else {
            setError(event.payload.message || 'Execution failed')
          }
        }
      )
      unlisteners.push(unlistenDone)

      // Error event
      const unlistenError = await listen<{ message: string }>(
        'error',
        (event) => {
          setError(event.payload.message)
        }
      )
      unlisteners.push(unlistenError)
    }

    setupListeners()

    return () => {
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [setPlan, setState, setStepIndex, setError])
}
