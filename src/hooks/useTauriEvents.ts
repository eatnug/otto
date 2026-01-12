import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
import { useOttoStore } from '../store/otto'
import type { ActionPlan, AgentSession, AgentSessionV2, ScreenState, AtomicAction, ActionResult, VerificationResult, DecompositionInfo, LlmDebugEvent, LlmResponseEvent, ToolResult } from '../types'

const INPUT_HEIGHT = 88
const STEP_HEIGHT = 48

// Calculate and apply window height directly
async function updateWindowHeight() {
  const storeState = useOttoStore.getState()
  const { agentSession, agentSessionV2, goalPipelineStates, decompositionInfo, useAgentMode, useAgentV2, state, plan } = storeState
  const goals = agentSession?.goals || []
  const planSteps = agentSessionV2?.plan?.steps || []
  const legacySteps = plan?.steps || []

  let height = INPUT_HEIGHT

  // V2 Agent mode (tool-based)
  if (useAgentV2 && agentSessionV2) {
    // Plan steps height
    height += planSteps.length * STEP_HEIGHT
    // Status row
    if (agentSessionV2.state === 'done' || agentSessionV2.state === 'failed') {
      height += STEP_HEIGHT
    }
    // Extra padding during execution
    if (agentSessionV2.state === 'executing') {
      height += 24 // For step counter
    }
  }
  // Legacy mode states
  else if (state === 'planning') {
    height += STEP_HEIGHT
  } else if (state === 'error' && !useAgentMode) {
    height += STEP_HEIGHT
  } else if ((state === 'executing' || state === 'done') && !useAgentMode) {
    height += legacySteps.length * STEP_HEIGHT
    if (state === 'done') height += STEP_HEIGHT
  }
  // V1 Agent mode (goal-based)
  else if (useAgentMode && goals.length > 0) {
    // Decomposition info height
    if (decompositionInfo) height += 56

    // Goals height
    const currentGoalIndex = agentSession?.current_goal_index ?? -1
    for (let idx = 0; idx < goals.length; idx++) {
      const goal = goals[idx]
      const pipeline = goalPipelineStates[goal.id]
      const isCurrent = idx === currentGoalIndex && goal.status === 'in_progress'
      const isCompleted = goal.status === 'completed'
      const isFailed = goal.status === 'failed'
      const shouldShowPipeline = pipeline && (isCurrent || isCompleted || isFailed)

      if (shouldShowPipeline) {
        let pipelineRows = 1
        if (pipeline.observation) pipelineRows++
        if (pipeline.actionType) pipelineRows++
        if (pipeline.actionResult) pipelineRows++
        if (pipeline.verification) pipelineRows++
        height += 48 + (pipelineRows * 22) + 20
      } else {
        height += STEP_HEIGHT
      }
    }

    // Done/error row
    if (agentSession?.state === 'complete' || agentSession?.state === 'error') {
      height += STEP_HEIGHT
    }
  }

  try {
    const window = getCurrentWindow()
    await window.setSize(new LogicalSize(680, height))
    console.log('[HEIGHT] Resize to:', height)
  } catch (err) {
    console.error('Failed to resize:', err)
  }
}

export function useTauriEvents() {
  const {
    setPlan, setState, setStepIndex, setError, setDebugLog,
    setAgentSession, updateGoalPipeline, setDecompositionInfo,
    setAgentSessionV2,
    addLlmPrompt, addLlmResponse
  } = useOttoStore()

  useEffect(() => {
    const unlisteners: (() => void)[] = []

    // Subscribe to store changes to detect reset
    const unsubscribe = useOttoStore.subscribe((state, prevState) => {
      // Detect reset: state went from non-idle to idle
      if (state.state === 'idle' && prevState.state !== 'idle') {
        updateWindowHeight()
      }
    })

    const setupListeners = async () => {
      // Set initial window size
      updateWindowHeight()

      // ============================================
      // Legacy Events (for backwards compatibility)
      // ============================================

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

      // ============================================
      // New Agent Events
      // ============================================

      // Helper to get current goal ID
      const getCurrentGoalId = () => {
        const session = useOttoStore.getState().agentSession
        if (!session) return null
        const goal = session.goals[session.current_goal_index]
        return goal?.id ?? null
      }

      // Decomposition info - how command was parsed
      const unlistenDecomposition = await listen<DecompositionInfo>(
        'decomposition',
        (event) => {
          setDecompositionInfo(event.payload)
          setTimeout(() => updateWindowHeight(), 10)
        }
      )
      unlisteners.push(unlistenDecomposition)

      // Goals ready - resize to show goals
      const unlistenGoalsReady = await listen('goals_ready', () => {
        setTimeout(() => updateWindowHeight(), 10)
      })
      unlisteners.push(unlistenGoalsReady)

      // Agent session update (main state sync)
      // Handles both v1 (goal-based) and v2 (tool-based) formats
      const unlistenAgentSession = await listen<AgentSession | AgentSessionV2>(
        'agent_session',
        (event) => {
          const payload = event.payload as unknown as Record<string, unknown>

          // Detect v2 format by checking for 'task' field (v1 has 'original_command')
          if ('task' in payload && !('original_command' in payload)) {
            // V2 format
            const v2Session = event.payload as AgentSessionV2
            console.log('[V2] Agent session update:', v2Session.state)
            setAgentSessionV2(v2Session)

            // Update global state for UI
            if (v2Session.state === 'done') {
              setState('done')
            } else if (v2Session.state === 'failed') {
              setState('error')
            } else if (v2Session.state === 'executing') {
              setState('executing')
            } else if (v2Session.state === 'planning') {
              setState('planning')
            }
          } else {
            // V1 format
            const v1Session = event.payload as AgentSession
            setAgentSession(v1Session)

            // Update pipeline step based on agent state
            const goalId = v1Session.goals[v1Session.current_goal_index]?.id
            if (goalId) {
              const stateToStep: Record<string, 'observing' | 'thinking' | 'acting' | 'verifying' | 'done'> = {
                observing: 'observing',
                thinking: 'thinking',
                acting: 'acting',
                verifying: 'verifying',
                complete: 'done',
              }
              const step = stateToStep[v1Session.state]
              if (step) {
                updateGoalPipeline(goalId, { step })
              }
            }
            // Also update global state for UI consistency
            if (v1Session.state === 'complete') {
              setState('done')
            } else if (v1Session.state === 'error') {
              setState('error')
            }
          }

          // Resize window immediately
          setTimeout(() => updateWindowHeight(), 10)
        }
      )
      unlisteners.push(unlistenAgentSession)

      // Goal started - initialize pipeline state
      const unlistenGoalStarted = await listen<{ goalIndex: number }>(
        'goal_started',
        (event) => {
          const session = useOttoStore.getState().agentSession
          const goalId = session?.goals[event.payload.goalIndex]?.id
          if (goalId) {
            updateGoalPipeline(goalId, {
              step: 'observing',
              observation: undefined,
              actionType: undefined,
              actionParams: undefined,
              actionRationale: undefined,
              actionResult: undefined,
              actionError: undefined,
              verification: undefined,
              verified: undefined,
            })
            setTimeout(() => updateWindowHeight(), 10)
          }
        }
      )
      unlisteners.push(unlistenGoalStarted)

      // Goal completed
      const unlistenGoalCompleted = await listen<{ goalIndex: number }>(
        'goal_completed',
        (event) => {
          const session = useOttoStore.getState().agentSession
          const goalId = session?.goals[event.payload.goalIndex]?.id
          if (goalId) {
            updateGoalPipeline(goalId, { step: 'done' })
            setTimeout(() => updateWindowHeight(), 10)
          }
        }
      )
      unlisteners.push(unlistenGoalCompleted)

      // Screen observation - update pipeline with observation
      const unlistenObservation = await listen<ScreenState>(
        'observation',
        (event) => {
          const goalId = getCurrentGoalId()
          if (goalId) {
            updateGoalPipeline(goalId, {
              step: 'thinking',
              observation: event.payload.description,
            })
            setTimeout(() => updateWindowHeight(), 10)
          }
        }
      )
      unlisteners.push(unlistenObservation)

      // Action planned - update pipeline with full action details
      const unlistenActionPlanned = await listen<AtomicAction>(
        'action_planned',
        (event) => {
          const goalId = getCurrentGoalId()
          if (goalId) {
            // Format params for display
            const params = event.payload.params
            let paramsStr = ''
            if ('app_name' in params) paramsStr = params.app_name
            else if ('text' in params) paramsStr = `"${params.text}"`
            else if ('key' in params) {
              const mods = params.modifiers?.join('+') || ''
              paramsStr = mods ? `${mods}+${params.key}` : params.key
            }
            else if ('x' in params && 'y' in params) paramsStr = `(${params.x}, ${params.y})`
            else if ('ms' in params) paramsStr = `${params.ms}ms`
            else if ('element' in params) paramsStr = params.element

            updateGoalPipeline(goalId, {
              step: 'acting',
              actionType: event.payload.action_type,
              actionParams: paramsStr,
              actionRationale: event.payload.rationale,
            })
            setTimeout(() => updateWindowHeight(), 10)
          }
        }
      )
      unlisteners.push(unlistenActionPlanned)

      // Action completed - update pipeline with result
      const unlistenActionCompleted = await listen<ActionResult>(
        'action_completed',
        (event) => {
          const goalId = getCurrentGoalId()
          if (goalId) {
            updateGoalPipeline(goalId, {
              step: 'verifying',
              actionResult: event.payload.success ? 'success' : 'failed',
              actionError: event.payload.error_message,
            })
            setTimeout(() => updateWindowHeight(), 10)
          }
        }
      )
      unlisteners.push(unlistenActionCompleted)

      // Verification result
      const unlistenVerification = await listen<VerificationResult>(
        'verification',
        (event) => {
          const goalId = getCurrentGoalId()
          if (goalId) {
            updateGoalPipeline(goalId, {
              verification: event.payload.observation,
              verified: event.payload.goal_achieved,
            })
          }
        }
      )
      unlisteners.push(unlistenVerification)

      // Session complete
      const unlistenSessionComplete = await listen(
        'session_complete',
        () => {
          setState('done')
        }
      )
      unlisteners.push(unlistenSessionComplete)

      // Agent error
      const unlistenAgentError = await listen<{ message: string }>(
        'agent_error',
        (event) => {
          setError(event.payload.message)
        }
      )
      unlisteners.push(unlistenAgentError)

      // ============================================
      // LLM Debug Events
      // ============================================

      // LLM prompt sent
      const unlistenLlmPrompt = await listen<LlmDebugEvent>(
        'llm_prompt',
        (event) => {
          addLlmPrompt(event.payload)
        }
      )
      unlisteners.push(unlistenLlmPrompt)

      // LLM response received
      const unlistenLlmResponse = await listen<LlmResponseEvent>(
        'llm_response',
        (event) => {
          addLlmResponse(event.payload)
        }
      )
      unlisteners.push(unlistenLlmResponse)

      // ============================================
      // V2 Agent Events (Tool-based)
      // ============================================

      // V2 agent uses the same 'agent_session' event name but with different payload
      // The listener above handles v1 format, we need to detect v2 format
      // V2 format has: { id, task, state, plan, step_count, error }
      // V1 format has: { id, original_command, goals, ... }

      // We'll add a separate check in the agent_session listener
      // For now, the v2 agent also emits 'agent_session' - we detect by checking for 'task' field

      // Agent done (v2)
      const unlistenAgentDone = await listen<string>(
        'agent_done',
        (event) => {
          console.log('[V2] Agent done:', event.payload)
          setState('done')
          setTimeout(() => updateWindowHeight(), 10)
        }
      )
      unlisteners.push(unlistenAgentDone)

      // Agent failed (v2)
      const unlistenAgentFailed = await listen<string>(
        'agent_failed',
        (event) => {
          console.log('[V2] Agent failed:', event.payload)
          setError(event.payload)
          setTimeout(() => updateWindowHeight(), 10)
        }
      )
      unlisteners.push(unlistenAgentFailed)

      // Tool result (v2) - for debugging
      const unlistenToolResult = await listen<ToolResult>(
        'tool_result',
        (event) => {
          console.log('[V2] Tool result:', event.payload)
        }
      )
      unlisteners.push(unlistenToolResult)
    }

    setupListeners()

    return () => {
      unlisteners.forEach((unlisten) => unlisten())
      unsubscribe()
    }
  }, [setPlan, setState, setStepIndex, setError, setDebugLog, setAgentSession, updateGoalPipeline, setDecompositionInfo, setAgentSessionV2, addLlmPrompt, addLlmResponse])
}
