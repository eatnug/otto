import { useState, useCallback, KeyboardEvent, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useOttoStore } from '../store/otto'

interface Props {
  disabled?: boolean
}

export function CommandInput({ disabled }: Props) {
  const [input, setInput] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const { setCommand, setState, state } = useOttoStore()

  // Clear input when returning to idle
  useEffect(() => {
    if (state === 'idle') {
      setInput('')
      inputRef.current?.focus()
    }
  }, [state])

  const handleSubmit = useCallback(async () => {
    if (!input.trim() || disabled) return

    setCommand(input)
    setState('planning')

    try {
      await invoke('plan_command', { command: input })
    } catch (err) {
      console.error('Failed to plan command:', err)
    }
  }, [input, setCommand, setState, disabled])

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter' && !disabled) {
        handleSubmit()
      } else if (e.key === 'Escape') {
        invoke('hide_window')
      }
    },
    [handleSubmit, disabled]
  )

  return (
    <div className="command-input">
      <input
        ref={inputRef}
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="What would you like me to do?"
        autoFocus
        disabled={disabled}
      />
    </div>
  )
}
