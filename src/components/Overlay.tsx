import { useEffect, useState, useCallback } from 'react'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
import { useOttoStore } from '../store/otto'
import { useTauriEvents } from '../hooks/useTauriEvents'
import { CommandInput } from './CommandInput'

const INPUT_HEIGHT = 88 // 68 + 20 for drag handle
const STEP_HEIGHT = 48

export function Overlay() {
  useTauriEvents()
  const { state, error, plan, currentStepIndex, reset, debugLogs } = useOttoStore()
  const [windowHeight, setWindowHeight] = useState(INPUT_HEIGHT)

  const steps = plan?.steps || []

  // Calculate and apply window height
  useEffect(() => {
    const calculateHeight = () => {
      if (state === 'idle') {
        return INPUT_HEIGHT
      }
      if (state === 'planning') {
        return INPUT_HEIGHT + STEP_HEIGHT // Show "Processing..."
      }
      if (state === 'error') {
        return INPUT_HEIGHT + STEP_HEIGHT
      }
      // executing or done: show all steps + done row
      const stepsHeight = steps.length * STEP_HEIGHT
      const doneHeight = state === 'done' ? STEP_HEIGHT : 0
      return INPUT_HEIGHT + stepsHeight + doneHeight
    }

    const newHeight = calculateHeight()
    setWindowHeight(newHeight)

    const resize = async () => {
      try {
        const window = getCurrentWindow()
        console.log('Resizing window to height:', newHeight)
        await window.setSize(new LogicalSize(680, newHeight))
      } catch (err) {
        console.error('Failed to resize window:', err)
      }
    }
    resize()
  }, [state, steps.length])

  const handleConfirm = () => {
    reset()
  }

  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    // Don't drag if clicking on interactive elements
    const target = e.target as HTMLElement
    if (target.tagName === 'INPUT' || target.tagName === 'BUTTON') {
      return
    }
    await getCurrentWindow().startDragging()
  }, [])

  return (
    <div className="overlay" style={{ height: windowHeight }}>
      <div className="drag-handle" onMouseDown={handleMouseDown}>
        <div className="drag-indicator" />
      </div>
      <CommandInput disabled={state === 'planning' || state === 'executing'} />

      <div className="results">
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
      </div>
    </div>
  )
}
