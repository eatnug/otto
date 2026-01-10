import { useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useOttoStore } from '../store/otto'

export function ProgressView() {
  const { plan, currentStepIndex, state } = useOttoStore()

  const handleCancel = useCallback(async () => {
    try {
      await invoke('cancel_execution')
    } catch (err) {
      console.error('Failed to cancel execution:', err)
    }
  }, [])

  if (!plan) return null

  const progress = ((currentStepIndex + 1) / plan.steps.length) * 100

  return (
    <div className="progress-view">
      <div className="progress-header">
        <h3>Executing...</h3>
        <span className="progress-text">
          Step {currentStepIndex + 1} of {plan.steps.length}
        </span>
      </div>

      <div className="progress-bar">
        <div className="progress-fill" style={{ width: `${progress}%` }} />
      </div>

      <div className="progress-steps">
        {plan.steps.map((step, index) => (
          <div
            key={step.id}
            className={`progress-step ${
              index < currentStepIndex
                ? 'completed'
                : index === currentStepIndex
                  ? 'current'
                  : 'pending'
            }`}
          >
            <span className="step-indicator">
              {index < currentStepIndex ? '\u2713' : index + 1}
            </span>
            <span className="step-description">{step.description}</span>
          </div>
        ))}
      </div>

      {state === 'executing' && (
        <div className="progress-actions">
          <button className="btn-cancel" onClick={handleCancel}>
            Cancel
          </button>
        </div>
      )}
    </div>
  )
}
