
export type ToWorkerMessage =
    { checkSupport: { className: string } }

export type ToMainMessage =
    { checkSupport: { supported: boolean } }