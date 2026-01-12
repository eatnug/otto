import { useCallback } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useOttoStore } from '../store/otto'
import { useTauriEvents } from '../hooks/useTauriEvents'
import { CommandInput } from './CommandInput'

export function Overlay() {
  useTauriEvents()
  const {
    state, error, plan, currentStepIndex, reset, debugLogs,
    agentSession, useAgentMode, goalPipelineStates, decompositionInfo
  } = useOttoStore()

  const steps = plan?.steps || []
  const goals = agentSession?.goals || []

  const handleConfirm = () => {
    reset()
  }

  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement
    if (target.tagName === 'INPUT' || target.tagName === 'BUTTON') {
      return
    }
    await getCurrentWindow().startDragging()
  }, [])

  // Pipeline step labels
  const pipelineStepLabels: Record<string, string> = {
    observing: '👁 Observing',
    thinking: '🧠 Thinking',
    acting: '⚡ Acting',
    verifying: '✓ Verifying',
    done: '✓ Done',
  }

  // Render agent mode UI
  const renderAgentMode = () => {
    if (!agentSession) return null

    const currentGoalIndex = agentSession.current_goal_index
    const agentState = agentSession.state

    return (
      <>
        {/* Decomposition info */}
        {decompositionInfo && (
          <div className="decomposition-info">
            <div className="decomposition-header">
              <span className="decomposition-method">
                {decompositionInfo.method === 'pattern' ? '⚡ Pattern Match' : '🤖 LLM Decomposed'}
              </span>
              {decompositionInfo.pattern_name && (
                <span className="decomposition-pattern">{decompositionInfo.pattern_name}</span>
              )}
            </div>
            <div className="decomposition-command">
              "{decompositionInfo.original_command}" → {goals.length} tasks
            </div>
          </div>
        )}

        {/* Goals list */}
        {goals.map((goal, idx) => {
          const isCompleted = goal.status === 'completed'
          const isFailed = goal.status === 'failed'
          const isCurrent = idx === currentGoalIndex && goal.status === 'in_progress'
          const isPending = goal.status === 'pending'
          const pipeline = goalPipelineStates[goal.id]

          return (
            <div
              key={goal.id}
              className={`goal-row ${isCompleted ? 'completed' : ''} ${isCurrent ? 'current' : ''} ${isFailed ? 'failed' : ''}`}
            >
              <div className="goal-header">
                {isCurrent && <div className="spinner" />}
                {isCompleted && <span className="step-icon success">✓</span>}
                {isFailed && <span className="step-icon error">✕</span>}
                {isPending && <span className="step-icon pending">○</span>}
                <span className="goal-text">{goal.description}</span>
                {isCurrent && goal.attempts > 0 && (
                  <span className="retry-badge">retry {goal.attempts}</span>
                )}
              </div>

              {/* Pipeline details - show for current and completed goals */}
              {pipeline && (isCurrent || isCompleted || isFailed) && (
                <div className="pipeline-info">
                  {/* Step indicator */}
                  <div className="pipeline-header">
                    <span className="pipeline-step">{pipelineStepLabels[pipeline.step] || pipeline.step}</span>
                  </div>

                  {/* Observe: what did vision see */}
                  {pipeline.observation && (
                    <div className="pipeline-row">
                      <span className="pipeline-label">👁 Observed:</span>
                      <span className="pipeline-value">{pipeline.observation.slice(0, 80)}</span>
                    </div>
                  )}

                  {/* Think: what action was decided */}
                  {pipeline.actionType && (
                    <div className="pipeline-row">
                      <span className="pipeline-label">🧠 Decided:</span>
                      <span className="pipeline-value">
                        <span className="action-type">{pipeline.actionType}</span>
                        {pipeline.actionParams && <span className="action-params">{pipeline.actionParams}</span>}
                      </span>
                    </div>
                  )}

                  {/* Act: execution result */}
                  {pipeline.actionResult && (
                    <div className="pipeline-row">
                      <span className="pipeline-label">⚡ Result:</span>
                      <span className={`pipeline-value ${pipeline.actionResult}`}>
                        {pipeline.actionResult === 'success' ? '✓ Success' : `✕ Failed${pipeline.actionError ? `: ${pipeline.actionError}` : ''}`}
                      </span>
                    </div>
                  )}

                  {/* Verify: did it work */}
                  {pipeline.verification && (
                    <div className="pipeline-row">
                      <span className="pipeline-label">✓ Verified:</span>
                      <span className={`pipeline-value ${pipeline.verified ? 'success' : 'failed'}`}>
                        {pipeline.verified ? '✓ Goal achieved' : pipeline.verification}
                      </span>
                    </div>
                  )}
                </div>
              )}
            </div>
          )
        })}

        {/* Error state */}
        {agentState === 'error' && agentSession.error && (
          <div className="step-row error">
            <span className="step-icon">✕</span>
            <span className="step-text">{agentSession.error}</span>
            <button className="btn-ok" onClick={handleConfirm}>OK</button>
          </div>
        )}

        {/* Done state */}
        {agentState === 'complete' && (
          <div className="step-row done">
            <span className="step-icon success">✓</span>
            <span className="step-text">All goals completed</span>
            <button className="btn-ok" onClick={handleConfirm}>OK</button>
          </div>
        )}
      </>
    )
  }

  // Render legacy mode UI
  const renderLegacyMode = () => (
    <>
      {state === 'planning' && (
        <div className="step-row">
          <div className="spinner" />
          <span>Processing...</span>
        </div>
      )}

      {state === 'error' && (
        <div className="step-row error">
          <span className="step-icon">✕</span>
          <span className="step-text">{error}</span>
          <button className="btn-ok" onClick={handleConfirm}>OK</button>
        </div>
      )}

      {(state === 'executing' || state === 'done') && steps.map((step, idx) => {
        const isCompleted = idx < currentStepIndex || state === 'done'
        const isCurrent = idx === currentStepIndex && state === 'executing'
        const debugLog = debugLogs[idx]

        return (
          <div
            key={step.id}
            className={`step-row ${isCompleted ? 'completed' : ''} ${isCurrent ? 'current' : ''}`}
          >
            {isCurrent && <div className="spinner" />}
            {isCompleted && <span className="step-icon success">✓</span>}
            {!isCurrent && !isCompleted && <span className="step-icon pending">○</span>}
            <span className="step-text">
              {step.description}
              {debugLog && <span className="debug-info"> [{debugLog}]</span>}
            </span>
          </div>
        )
      })}

      {state === 'done' && (
        <div className="step-row done">
          <span className="step-icon success">✓</span>
          <span className="step-text">Complete</span>
          <button className="btn-ok" onClick={handleConfirm}>OK</button>
        </div>
      )}
    </>
  )

  return (
    <div className="overlay">
      <div className="drag-handle" onMouseDown={handleMouseDown}>
        <div className="drag-indicator" />
      </div>
      <CommandInput disabled={
        state === 'planning' ||
        state === 'executing' ||
        (useAgentMode && agentSession !== null && agentSession.state !== 'complete' && agentSession.state !== 'error')
      } />

      <div className="results">
        {useAgentMode && agentSession ? renderAgentMode() : renderLegacyMode()}
      </div>
    </div>
  )
}
