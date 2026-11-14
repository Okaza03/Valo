import { CheckIcon, CopyIcon } from "lucide-react"
import { AnimatePresence, motion } from "motion/react"
import { useState } from "react"
import { toast } from "sonner"
import { InputGroupButton } from "@/components/ui/input-group"

/** Copies `text`; the icon flips to a check for a moment. */
export function CopyButton({ text, label = "Copy" }: { text: string; label?: string }) {
  const [done, setDone] = useState(false)
  async function copy() {
    try {
      await navigator.clipboard.writeText(text)
      setDone(true)
      toast.success("Copied to the clipboard")
      setTimeout(() => setDone(false), 1800)
    } catch {
      toast.error("Couldn't copy: select the text and copy it")
    }
  }
  return (
    <InputGroupButton size="icon-xs" aria-label={label} title={label} onClick={copy}>
      <AnimatePresence mode="wait" initial={false}>
        <motion.span
          key={done ? "done" : "copy"}
          initial={{ scale: 0.4, opacity: 0, rotate: -30 }}
          animate={{ scale: 1, opacity: 1, rotate: 0 }}
          exit={{ scale: 0.4, opacity: 0, rotate: 30 }}
          transition={{ duration: 0.15 }}
          className="grid place-items-center"
        >
          {done ? <CheckIcon className="text-emerald-400" /> : <CopyIcon />}
        </motion.span>
      </AnimatePresence>
    </InputGroupButton>
  )
}
