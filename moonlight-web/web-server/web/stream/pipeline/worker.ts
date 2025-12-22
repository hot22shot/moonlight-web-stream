import { ToMainMessage, ToWorkerMessage } from "./worker_types.js"

function onMessage(message: ToWorkerMessage) {
    if ("checkSupport" in message) {
        const className = message.checkSupport.className

        const supported = className in self

        const response: ToMainMessage = {
            checkSupport: { supported }
        }
        postMessage(response)
    }
}

onmessage = (event) => {
    const message = event.data as ToWorkerMessage
    onMessage(message)
}