import { StreamSupportedVideoFormats } from "../../api_bindings.js";
import { DataVideoRenderer, FrameVideoRenderer, VideoDecodeUnit, VideoRendererSetup } from "./index.js";

const CODEC_TRANSLATION: Record<keyof typeof StreamSupportedVideoFormats, string> = {
    // H.264 / AVC
    H264: "avc1.64001E",              // High Profile, Level 3.0
    H264_HIGH8_444: "avc1.6E001E",    // High 4:4:4 Predictive, 8-bit

    // H.265 / HEVC
    H265: "hev1.1.6.L120.90",         // Main profile, 8-bit
    H265_MAIN10: "hev1.2.6.L120.90",  // Main10 profile
    H265_REXT8_444: "hev1.3.6.L120.90",   // Range Extensions, 4:4:4 8-bit
    H265_REXT10_444: "hev1.4.6.L120.90",  // Range Extensions, 4:4:4 10-bit

    // AV1
    AV1_MAIN8: "av01.0.08M.08",       // Main profile, 8-bit
    AV1_MAIN10: "av01.0.10M.10",      // Main profile, 10-bit
    AV1_HIGH8_444: "av01.1.08M.08",   // High profile, 4:4:4 8-bit
    AV1_HIGH10_444: "av01.1.10M.10",  // High profile, 4:4:4 10-bit
};

const START_CODE_SHORT = new Uint8Array([0x00, 0x00, 0x01]); // 3-byte start code
const START_CODE_LONG = new Uint8Array([0x00, 0x00, 0x00, 0x01]); // 4-byte start code
function startsWith(buffer: Uint8Array, position: number, check: Uint8Array): boolean {
    for (let i = 0; i < check.length; i++) {
        if (buffer[position + i] != check[i]) {
            return false
        }
    }
    return true
}

export class VideoDecoderPipe<T extends FrameVideoRenderer> extends DataVideoRenderer {

    static isBrowserSupported(): boolean {
        // TODO: check for config

        // We need the WebCodecs API
        return "VideoDecoder" in window
    }

    private base: T

    private codec: "h264" | "h265" | "av1" | null = null
    private decoder: VideoDecoder

    constructor(base: T) {
        super(`data_to_frame -> ${base.implementationName}`)
        this.base = base

        this.decoder = new VideoDecoder({
            error: this.onError.bind(this),
            output: this.onOutput.bind(this)
        })
    }

    private onError(error: any) {
        // TODO: use logger
        console.error(error)
    }

    private onOutput(frame: VideoFrame) {
        this.base.submitFrame(frame)
    }

    private decoderConfig: VideoDecoderConfig | null = null
    setup(setup: VideoRendererSetup): void {
        this.decoderConfig = {
            codec: CODEC_TRANSLATION[setup.format],
            codedWidth: setup.width,
            codedHeight: setup.height,
            hardwareAcceleration: "prefer-hardware",
            optimizeForLatency: true
        }
        this.decoder.configure(this.decoderConfig)

        if (setup.format == "H264" || setup.format == "H264_HIGH8_444") {
            this.codec = "h264"
        } else if (setup.format == "H265" || setup.format == "H265_MAIN10" || setup.format == "H265_REXT8_444" || setup.format == "H265_REXT10_444") {
            this.codec = "h265"
        } else if (setup.format == "AV1_MAIN8" || setup.format == "AV1_MAIN10" || setup.format == "AV1_HIGH8_444" || setup.format == "AV1_HIGH10_444") {
            this.codec = "av1"
        }

        this.base.setup(setup)
    }

    private currentUnitSize = 0
    private currentUnit: Uint8Array = new Uint8Array(1000)
    submitDecodeUnit(unit: VideoDecodeUnit): void {
        // We're getting annex prefixed nalus but we need length prefixed nalus -> convert them

        const data = new Uint8Array(unit.data);

        let unitBegin = 0
        let currentPosition = 0

        while (currentPosition < data.length) {
            let startCodeLength = 0
            let foundStartCode = false

            if (startsWith(data, currentPosition, START_CODE_LONG)) {
                startCodeLength = START_CODE_LONG.length
                foundStartCode = true
            } else if (startsWith(data, currentPosition, START_CODE_SHORT)) {
                startCodeLength = START_CODE_SHORT.length
                foundStartCode = true
            }

            if (foundStartCode) {
                // all previous data should go into the currentUnit and be submitted
                const slice = data.subarray(unitBegin, currentPosition)
                this.checkUnitBufferSize(slice.length)

                this.currentUnit.set(slice, this.currentUnitSize)
                this.currentUnitSize += slice.length

                this.submitUnit(unit.timestampMicroseconds, unit.durationMicroseconds)

                currentPosition += startCodeLength;
                unitBegin = currentPosition
            } else {
                currentPosition += 1;
            }
        }

        const slice = data.subarray(unitBegin, currentPosition)
        this.checkUnitBufferSize(slice.length)
        this.currentUnit.set(slice)
        this.currentUnitSize = slice.length
    }
    private checkUnitBufferSize(requiredExtra: number) {
        if (this.currentUnitSize + requiredExtra > this.currentUnit.length) {
            const newUnit = new Uint8Array((this.currentUnitSize + requiredExtra) * 2);

            newUnit.set(this.currentUnit);
            this.currentUnit = newUnit;
        }
    }
    private getCurrentNalUnitType(): number {
        if (this.codec == "h264") {
            // get the Nal header and read the type
            // https://datatracker.ietf.org/doc/html/rfc3984#section-1.3

            const header = this.currentUnit[0]
            let nalUnitType = header & 0b00011111;

            return nalUnitType
        } else if (this.codec == "h265") {
            // TODO
            throw "Cannot find out if the submitted frame is a key frame because the codec \"h265\" is not yet implemented"
        } else if (this.codec == "av1") {
            // TODO
            throw "Cannot find out if the submitted frame is a key frame because the codec \"av1\" is not yet implemented"
        } else {
            // TODO
            throw "Cannot find out if the submitted frame is a key frame because no codec was set"
        }
    }

    private hadKeyFrame = false

    // TODO: put this into a seperated class for each codec?
    // private sps: ArrayBuffer | null
    // private pps: ArrayBuffer | null
    // private idr: ArrayBuffer | null

    private submitUnit(timestampMicroseconds: number, durationMicroseconds: number) {
        if (this.currentUnitSize == 0) {
            return
        }
        const nalType = this.getCurrentNalUnitType()

        if (nalType == 5) {
            // IDR
            // this.idr = 
        }

        // if (isKey) {
        //     this.hadKeyFrame = true
        // }

        // if (!this.hadKeyFrame && !isKey) {
        //     console.debug("Not submitting delta frame because no key frame was present")
        //     return
        // }

        // const data = this.currentUnit.slice(0, this.currentUnitSize)
        // this.currentUnitSize = 0

        // // TODO: remove
        // console.info("Frame", isKey, data)

        // const chunk = new EncodedVideoChunk({
        //     type: isKey ? "key" : "delta",
        //     timestamp: timestampMicroseconds,
        //     duration: durationMicroseconds,
        //     data,
        // })

        // this.decoder.decode(chunk)
    }

    cleanup(): void {
        this.decoder.close()

        this.base.cleanup()
    }

    onUserInteraction(): void {
        this.base.onUserInteraction()
    }

    getStreamRect(): DOMRect {
        return this.base.getStreamRect()
    }

    mount(parent: HTMLElement): void {
        this.base.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        this.base.unmount(parent)
    }

}