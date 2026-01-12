import { useCallback, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useOttoStore } from '../store/otto'
import { useTauriEvents } from '../hooks/useTauriEvents'
import { CommandInput } from './CommandInput'
import type { LlmCallType } from '../types'

// Format call type for display
function formatCallType(type: LlmCallType): string {
  const labels: Record<LlmCallType, string> = {
    decomposition: 'Decompose',
    screen_description: 'Vision',
    action_decision: 'Think',
    verification: 'Verify',
    find_element: 'Find',
  }
  return labels[type] || type
}

export function Overlay() {
  useTauriEvents()
  const {
    state, agentSession, agentSessionV2, useAgentMode, useAgentV2,
    llmCalls, selectedLlmCall, selectLlmCall, clearLlmCalls
  } = useOttoStore()

  const [expandedPrompt, setExpandedPrompt] = useState(false)
  const [expandedResponse, setExpandedResponse] = useState(false)

  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement
    if (target.tagName === 'INPUT' || target.tagName === 'BUTTON') {
      return
    }
    await getCurrentWindow().startDragging()
  }, [])

  // Get sorted LLM calls by timestamp
  const sortedLlmCalls = Object.values(llmCalls).sort((a, b) => a.timestamp - b.timestamp)
  const selectedCall = selectedLlmCall ? llmCalls[selectedLlmCall] : null

  // Render LLM call list
  const renderLlmCallList = () => (
    <div className="llm-call-list">
      <div className="llm-list-header">
        <span>LLM Calls ({sortedLlmCalls.length})</span>
        {sortedLlmCalls.length > 0 && (
          <button className="btn-clear" onClick={clearLlmCalls}>Clear</button>
        )}
      </div>
      <div className="llm-list-items">
        {sortedLlmCalls.map((call) => (
          <div
            key={call.id}
            className={`llm-call-item ${call.status} ${selectedLlmCall === call.id ? 'selected' : ''}`}
            onClick={() => selectLlmCall(call.id)}
          >
            <span className={`call-type-badge ${call.type}`}>{formatCallType(call.type)}</span>
            <span className="call-model">{call.model}</span>
            <span className={`call-status ${call.status}`}>
              {call.status === 'pending' ? '...' : call.status === 'success' ? '✓' : '✕'}
            </span>
            {call.duration_ms && (
              <span className="call-duration">{call.duration_ms}ms</span>
            )}
          </div>
        ))}
        {sortedLlmCalls.length === 0 && (
          <div className="llm-empty">No LLM calls yet</div>
        )}
      </div>
    </div>
  )

  // Render LLM call detail
  const renderLlmCallDetail = () => {
    if (!selectedCall) {
      return (
        <div className="llm-detail-empty">
          Select an LLM call to view details
        </div>
      )
    }

    const promptLines = selectedCall.prompt.split('\n')
    const responseLines = (selectedCall.raw_response || '').split('\n')
    const maxCollapsedLines = 10

    return (
      <div className="llm-call-detail">
        <div className="detail-header">
          <span className={`call-type-badge ${selectedCall.type}`}>{formatCallType(selectedCall.type)}</span>
          <span className="call-model">{selectedCall.model}</span>
          {selectedCall.duration_ms && (
            <span className="call-duration">{selectedCall.duration_ms}ms</span>
          )}
          <span className={`call-status-badge ${selectedCall.status}`}>
            {selectedCall.status}
          </span>
        </div>

        {/* Prompt */}
        <div className="detail-section">
          <div className="section-header">
            <span>Prompt</span>
            <div className="section-actions">
              <button onClick={() => navigator.clipboard.writeText(selectedCall.prompt)}>Copy</button>
              {promptLines.length > maxCollapsedLines && (
                <button onClick={() => setExpandedPrompt(!expandedPrompt)}>
                  {expandedPrompt ? 'Collapse' : 'Expand'}
                </button>
              )}
            </div>
          </div>
          <pre className="detail-content">
            {expandedPrompt
              ? selectedCall.prompt
              : promptLines.slice(0, maxCollapsedLines).join('\n')
            }
            {!expandedPrompt && promptLines.length > maxCollapsedLines && (
              <span className="more-indicator">
                {'\n'}... ({promptLines.length - maxCollapsedLines} more lines)
              </span>
            )}
          </pre>
        </div>

        {/* Response */}
        {selectedCall.raw_response !== undefined && (
          <div className="detail-section">
            <div className="section-header">
              <span>Response</span>
              <div className="section-actions">
                <button onClick={() => navigator.clipboard.writeText(selectedCall.raw_response || '')}>Copy</button>
                {responseLines.length > maxCollapsedLines && (
                  <button onClick={() => setExpandedResponse(!expandedResponse)}>
                    {expandedResponse ? 'Collapse' : 'Expand'}
                  </button>
                )}
              </div>
            </div>
            <pre className={`detail-content ${selectedCall.success ? 'success' : 'error'}`}>
              {expandedResponse
                ? selectedCall.raw_response
                : responseLines.slice(0, maxCollapsedLines).join('\n')
              }
              {!expandedResponse && responseLines.length > maxCollapsedLines && (
                <span className="more-indicator">
                  {'\n'}... ({responseLines.length - maxCollapsedLines} more lines)
                </span>
              )}
            </pre>
          </div>
        )}

        {/* Error */}
        {selectedCall.error && (
          <div className="detail-section error">
            <div className="section-header">Error</div>
            <pre className="detail-content error">{selectedCall.error}</pre>
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="overlay debug-mode">
      <div className="drag-handle" onMouseDown={handleMouseDown}>
        <div className="drag-indicator" />
      </div>
      <CommandInput disabled={
        state === 'planning' ||
        state === 'executing' ||
        (useAgentV2 && agentSessionV2 !== null && agentSessionV2.state !== 'done' && agentSessionV2.state !== 'failed') ||
        (useAgentMode && !useAgentV2 && agentSession !== null && agentSession.state !== 'complete' && agentSession.state !== 'error')
      } />

      {/* V2 Plan Progress */}
      {useAgentV2 && agentSessionV2?.plan && (
        <div className="plan-progress">
          {agentSessionV2.plan.steps.map((step, idx) => (
            <div
              key={step.id}
              className={`plan-step ${step.status} ${idx === agentSessionV2.plan!.current_step ? 'current' : ''}`}
            >
              <span className="step-number">{idx + 1}</span>
              <span className="step-desc">{step.description}</span>
              <span className={`step-status ${step.status}`}>
                {step.status === 'done' ? '✓' : step.status === 'in_progress' ? '...' : step.status === 'failed' ? '✕' : ''}
              </span>
            </div>
          ))}
          {agentSessionV2.state === 'executing' && (
            <div className="step-counter">Steps: {agentSessionV2.step_count}</div>
          )}
          {agentSessionV2.state === 'done' && (
            <div className="plan-done">Done</div>
          )}
          {agentSessionV2.state === 'failed' && agentSessionV2.error && (
            <div className="plan-error">{agentSessionV2.error}</div>
          )}
        </div>
      )}

      <div className="debug-container">
        {/* Left: LLM Call List */}
        {renderLlmCallList()}

        {/* Right: LLM Call Detail */}
        <div className="llm-detail-panel">
          {renderLlmCallDetail()}
        </div>
      </div>
    </div>
  )
}
