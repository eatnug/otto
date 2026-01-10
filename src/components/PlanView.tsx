import { useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useOttoStore } from '../store/otto'

export function PlanView() {
  const { plan, setState, reset } = useOttoStore()

  const handleExecute = useCallback(async () => {
    if (!plan) return
    setState('executing')
    try {
      await invoke('execute_plan', { plan })
    } catch (err) {
      console.error('Failed to execute plan:', err)
    }
  }, [plan, setState])

  const handleCancel = useCallback(() => {
    reset()
  }, [reset])

  if (!plan) return null

  return (
    <div className="plan-view">
      <div className="plan-header">
        <h3>Execution Plan</h3>
        <span className="command-text">{plan.original_command}</span>
      </div>

      <div className="plan-steps">
        {plan.steps.map((step, index) => (
          <div key={step.id} className="plan-step">
            <span className="step-number">{index + 1}</span>
            <span className="step-type">{step.type}</span>
            <span className="step-description">{step.description}</span>
          </div>
        ))}
      </div>

      {plan.requires_confirmation && (
        <div className="warning">
          This action may have irreversible effects. Please confirm.
        </div>
      )}

      <div className="plan-actions">
        <button className="btn-cancel" onClick={handleCancel}>
          Cancel
        </button>
        <button className="btn-execute" onClick={handleExecute}>
          Execute
        </button>
      </div>
    </div>
  )
}
