import { useState, useRef, useCallback } from 'react'
import { Upload, FileText, X, Loader2, CheckCircle, AlertCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import { documentsApi } from '@/api/endpoints/documents'

interface UploadDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onUploadComplete?: () => void
  documentType?: 'invoice' | 'contract'
}

type UploadState = 'idle' | 'uploading' | 'success' | 'error'

interface UploadResult {
  label: string
  name: string
  amount: string
}

export function UploadDialog({
  open,
  onOpenChange,
  onUploadComplete,
  documentType = 'invoice',
}: UploadDialogProps) {
  const [file, setFile] = useState<File | null>(null)
  const [dragOver, setDragOver] = useState(false)
  const [uploadState, setUploadState] = useState<UploadState>('idle')
  const [errorMessage, setErrorMessage] = useState('')
  const [uploadResult, setUploadResult] = useState<UploadResult | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const isContract = documentType === 'contract'
  const typeLabel = isContract ? 'Contract' : 'Invoice'

  const resetState = useCallback(() => {
    setFile(null)
    setUploadState('idle')
    setErrorMessage('')
    setUploadResult(null)
  }, [])

  const handleClose = useCallback(
    (isOpen: boolean) => {
      if (!isOpen) {
        resetState()
      }
      onOpenChange(isOpen)
    },
    [onOpenChange, resetState]
  )

  const handleFileSelect = useCallback((selectedFile: File) => {
    if (!selectedFile.name.toLowerCase().endsWith('.pdf')) {
      setErrorMessage('Only PDF files are supported')
      return
    }
    if (selectedFile.size > 50 * 1024 * 1024) {
      setErrorMessage('File size must be under 50 MB')
      return
    }
    setFile(selectedFile)
    setErrorMessage('')
    setUploadState('idle')
    setUploadResult(null)
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setDragOver(false)
      const droppedFile = e.dataTransfer.files[0]
      if (droppedFile) {
        handleFileSelect(droppedFile)
      }
    },
    [handleFileSelect]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(false)
  }, [])

  const handleUpload = useCallback(async () => {
    if (!file) return

    setUploadState('uploading')
    setErrorMessage('')

    try {
      if (isContract) {
        const result = await documentsApi.uploadContract(file)
        setUploadState('success')
        setUploadResult({
          label: result.contract_number || result.title || 'N/A',
          name: result.vendor_name || result.parties?.[0]?.name || 'Unknown',
          amount: result.total_value?.toString() || '0',
        })
      } else {
        const result = await documentsApi.uploadDocument(file)
        setUploadState('success')
        setUploadResult({
          label: result.invoice_number || 'N/A',
          name: result.vendor_name || result.vendor?.name || 'Unknown',
          amount: result.total_amount?.toString() || '0',
        })
      }
      onUploadComplete?.()
    } catch (err) {
      setUploadState('error')
      setErrorMessage(err instanceof Error ? err.message : 'Upload failed')
    }
  }, [file, isContract, onUploadComplete])

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Upload {typeLabel}</DialogTitle>
          <DialogDescription>
            Upload a PDF {typeLabel.toLowerCase()} for processing and analysis
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Drop Zone */}
          {uploadState !== 'success' && (
            <div
              onDrop={handleDrop}
              onDragOver={handleDragOver}
              onDragLeave={handleDragLeave}
              onClick={() => fileInputRef.current?.click()}
              className={cn(
                'flex cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed p-8 transition-colors',
                dragOver
                  ? 'border-accent bg-accent/5'
                  : 'border-primary-300 hover:border-primary-400 hover:bg-primary-50',
                uploadState === 'uploading' && 'pointer-events-none opacity-60'
              )}
            >
              <input
                ref={fileInputRef}
                type="file"
                accept=".pdf"
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0]
                  if (f) handleFileSelect(f)
                }}
              />
              <Upload className="mb-3 h-8 w-8 text-primary-400" />
              <p className="text-sm font-medium text-primary-700">
                Drop a PDF here or click to browse
              </p>
              <p className="mt-1 text-xs text-primary-400">PDF files up to 50 MB</p>
            </div>
          )}

          {/* Selected File */}
          {file && uploadState !== 'success' && (
            <div className="flex items-center gap-3 rounded-lg border border-primary-200 bg-primary-50 p-3">
              <FileText className="h-8 w-8 text-primary-500" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-primary-900">
                  {file.name}
                </p>
                <p className="text-xs text-primary-500">
                  {formatFileSize(file.size)}
                </p>
              </div>
              {uploadState === 'idle' && (
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    setFile(null)
                  }}
                  className="rounded p-1 hover:bg-primary-200"
                >
                  <X className="h-4 w-4 text-primary-500" />
                </button>
              )}
              {uploadState === 'uploading' && (
                <Loader2 className="h-5 w-5 animate-spin text-primary-500" />
              )}
            </div>
          )}

          {/* Success State */}
          {uploadState === 'success' && uploadResult && (
            <div className="rounded-lg border border-green-200 bg-green-50 p-4">
              <div className="flex items-center gap-2 text-green-700">
                <CheckCircle className="h-5 w-5" />
                <span className="font-medium">{typeLabel} processed</span>
              </div>
              <div className="mt-3 space-y-1 text-sm text-green-600">
                <p>
                  {isContract ? 'Contract' : 'Invoice'}:{' '}
                  <span className="font-medium">{uploadResult.label}</span>
                </p>
                <p>
                  Vendor: <span className="font-medium">{uploadResult.name}</span>
                </p>
                <p>
                  {isContract ? 'Value' : 'Amount'}:{' '}
                  <span className="font-medium">${uploadResult.amount}</span>
                </p>
              </div>
            </div>
          )}

          {/* Error State */}
          {errorMessage && (
            <div className="flex items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
              <AlertCircle className="h-4 w-4 shrink-0" />
              {errorMessage}
            </div>
          )}
        </div>

        <DialogFooter>
          {uploadState === 'success' ? (
            <Button onClick={() => handleClose(false)}>Done</Button>
          ) : (
            <>
              <Button variant="outline" onClick={() => handleClose(false)}>
                Cancel
              </Button>
              <Button
                onClick={handleUpload}
                disabled={!file || uploadState === 'uploading'}
              >
                {uploadState === 'uploading' ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Processing...
                  </>
                ) : (
                  <>
                    <Upload className="h-4 w-4" />
                    Upload
                  </>
                )}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
