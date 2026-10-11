import { Check, Copy, Download } from "lucide-react"
import { useEffect, useState } from "react"
import { Button } from "../../components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog"
import { Textarea } from "../../components/ui/textarea"
import { ToggleGroup, ToggleGroupItem } from "../../components/ui/toggle-group"
import { copyTextToClipboard } from "../../lib/clipboard"
import { downloadTextFile } from "../../lib/deck-export"
import { scanListCsv, scanListFileName, scanListText, type ScanEntry } from "./scan-list"

type ExportFormat = "txt" | "csv"

const FORMATS: Record<ExportFormat, { label: string; hint: string }> = {
  txt: {
    label: "Text",
    hint: "One line per card, like 2x Lightning Bolt (M10) 146 *F*. Paste it into a deck or another app.",
  },
  csv: {
    label: "CSV",
    hint: "Every detail: finish, language, Scryfall ID and price paid. Opens in a spreadsheet.",
  },
}

/** Copies or downloads the scanned list as decklist text or the collection import CSV. */
export function ScanExportDialog({
  open,
  entries,
  onClose,
}: {
  open: boolean
  entries: ScanEntry[]
  onClose: () => void
}) {
  const [format, setFormat] = useState<ExportFormat>("txt")
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle")
  const text = format === "csv" ? scanListCsv(entries) : scanListText(entries)

  useEffect(() => {
    if (copyState === "idle") return
    const timer = setTimeout(() => setCopyState("idle"), 2000)
    return () => clearTimeout(timer)
  }, [copyState])

  async function copy() {
    setCopyState((await copyTextToClipboard(text)) ? "copied" : "failed")
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="scan-sheet sm:max-w-lg" labelledBy="scan-export-title">
        <DialogHeader>
          <DialogTitle id="scan-export-title">Export scanned cards</DialogTitle>
          <DialogClose onClose={onClose} />
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-5 py-4">
          <ToggleGroup
            type="single"
            aria-label="Export format"
            value={format}
            onValueChange={(value) => {
              if (value === "txt" || value === "csv") setFormat(value)
            }}
            className="grid grid-cols-2 gap-1 rounded-btn border border-base-300 bg-base-100 p-1"
          >
            {(["txt", "csv"] as const).map((value) => (
              <ToggleGroupItem
                key={value}
                value={value}
                className="min-h-11 rounded-btn px-4 text-sm font-bold transition-colors hover:bg-base-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary data-[state=on]:bg-primary data-[state=on]:text-primary-content"
              >
                {FORMATS[value].label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          <p className="text-sm text-base-content/70">{FORMATS[format].hint}</p>
          <Textarea
            readOnly
            value={text}
            aria-label={`${FORMATS[format].label} export`}
            className="min-h-48 flex-1 whitespace-pre font-mono text-xs"
          />
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-base-300 px-5 py-3 pb-[calc(0.75rem_+_var(--safe-bottom))]">
          <Button type="button" variant="outline" onClick={() => void copy()}>
            {copyState === "copied" ? (
              <Check className="h-4 w-4" aria-hidden="true" />
            ) : (
              <Copy className="h-4 w-4" aria-hidden="true" />
            )}
            {copyState === "copied" ? "Copied" : copyState === "failed" ? "Copy failed" : "Copy"}
          </Button>
          <Button type="button" onClick={() => downloadTextFile(scanListFileName(format), text)}>
            <Download className="h-4 w-4" aria-hidden="true" />
            Download .{format}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
