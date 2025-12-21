import { createVideoWorker, TrackVideoRenderer, VideoRenderer, VideoRendererSetup } from "../video/index.js";

export class WorkerPipeline {

    private worker: Worker

    constructor() {
        this.worker = createVideoWorker()
    }
}

export class WorkerDataInput extends VideoRenderer {
    setup(setup: VideoRendererSetup): void {
        throw new Error("Method not implemented.");
    }
    cleanup(): void {
        throw new Error("Method not implemented.");
    }
    onUserInteraction(): void {
        throw new Error("Method not implemented.");
    }
    getStreamRect(): DOMRect {
        throw new Error("Method not implemented.");
    }
    mount(parent: HTMLElement): void {
        throw new Error("Method not implemented.");
    }
    unmount(parent: HTMLElement): void {
        throw new Error("Method not implemented.");
    }
}

export abstract class WorkerOutput<T extends VideoRenderer> extends VideoRenderer {
    static readonly type: string = "worker"
}

export function createWorkerPipe(input: "data", base: TrackVideoRenderer): void

export function createWorkerPipe(input: string, base: VideoRenderer) { }