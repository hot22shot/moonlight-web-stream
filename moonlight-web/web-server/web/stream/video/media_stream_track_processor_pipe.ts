import { FrameVideoRenderer, TrackVideoRenderer, VideoRendererSetup } from "./index.js";

export class MediaStreamTrackProcessorPipe<T extends FrameVideoRenderer> extends TrackVideoRenderer {
    static isBrowserSupported(): boolean {
        return "MediaStreamTrackProcessor" in window
    }

    private running: boolean = false
    private newProcessor: boolean = false
    private trackProcessor: MediaStreamTrackProcessor | null = null

    private base: T

    constructor(base: T) {
        super(`media_stream_track_processor -> ${base.implementationName}`)
        this.base = base
    }

    setTrack(track: MediaStreamTrack): void {
        this.trackProcessor = new MediaStreamTrackProcessor({ track })
        this.newProcessor = true
    }

    private async readTrack() {
        let reader: ReadableStreamDefaultReader<VideoFrame> | null = null

        while (this.running) {
            if (!reader || this.newProcessor) {
                this.newProcessor = false

                if (this.trackProcessor?.readable.locked) {
                    // Shouldn't happen
                    throw "Canvas video track processor is locked"
                }

                const newReader = this.trackProcessor?.readable.getReader()
                if (newReader) {
                    reader = newReader
                }
                await MediaStreamTrackProcessorPipe.wait(100)
                continue
            }

            // TODO: byob?
            const { done, value } = await reader.read()
            if (done) {
                console.error("Track Processor is done!")
                return
            }

            this.base.submitFrame(value)
        }
    }
    private static wait(time: number): Promise<void> {
        return new Promise((resolve, _reject) => {
            setTimeout(resolve, time)
        })
    }

    setup(setup: VideoRendererSetup): void {
        this.running = true
        this.readTrack()

        this.base.setup(setup)
    }
    cleanup(): void {
        this.running = false
        try {
            if (this.trackProcessor) {
                this.trackProcessor.readable.cancel()
            }
        } catch (e) {
            console.error(e)
        }
        this.trackProcessor = null

        this.base.cleanup()
    }

    onUserInteraction(): void {
        this.base.onUserInteraction()
    }

    mount(parent: HTMLElement): void {
        this.base.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        this.base.unmount(parent)
    }

    getStreamRect(): DOMRect {
        return this.base.getStreamRect()
    }

    getBase(): T {
        return this.base
    }
}