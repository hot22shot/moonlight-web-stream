import { getStreamRectCorrected, TrackVideoRenderer, VideoRendererInfo, VideoRendererSetup } from "./index.js";

export class VideoElementRenderer extends TrackVideoRenderer {
    static async getInfo(): Promise<VideoRendererInfo> {
        return {
            executionEnvironment: {
                main: "HTMLVideoElement" in window && "srcObject" in HTMLVideoElement.prototype,
                worker: false
            }
        }
    }

    private videoElement = document.createElement("video")
    private oldTrack: MediaStreamTrack | null = null
    private stream = new MediaStream()

    private size: [number, number] | null = null

    constructor() {
        super("video_element")

        this.videoElement.classList.add("video-stream")
        this.videoElement.preload = "none"
        this.videoElement.controls = false
        this.videoElement.autoplay = true
        this.videoElement.disablePictureInPicture = true
        this.videoElement.playsInline = true
        this.videoElement.muted = true

        if ("srcObject" in this.videoElement) {
            try {
                this.videoElement.srcObject = this.stream
            } catch (err: any) {
                if (err.name !== "TypeError") {
                    throw err;
                }

                console.error(err)
                throw `video_element renderer not supported: ${err}`
            }
        }
    }

    setup(setup: VideoRendererSetup): void {
        this.size = [setup.width, setup.height]
    }
    cleanup(): void {
        if (this.oldTrack) {
            this.stream.removeTrack(this.oldTrack)
        }
        this.videoElement.srcObject = null
    }

    setTrack(track: MediaStreamTrack): void {
        if (this.oldTrack) {
            this.stream.removeTrack(this.oldTrack)
        }

        this.stream.addTrack(track)
        this.oldTrack = track
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.videoElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.videoElement)
    }

    onUserInteraction(): void {
        if (this.videoElement.paused) {
            this.videoElement.play().then(() => {
                // Playing
            }).catch(error => {
                console.error(`Failed to play videoElement: ${error.message || error}`);
            })
        }
    }
    getStreamRect(): DOMRect {
        if (!this.size) {
            return new DOMRect()
        }

        return getStreamRectCorrected(this.videoElement.getBoundingClientRect(), this.size)
    }
}