import { ControllerConfig } from "../stream/gamepad.js";
import { Component, ComponentEvent } from "./index.js";
import { InputComponent, SelectComponent } from "./input.js";

export type StreamSettings = {
    bitrate: number
    packetSize: number
    videoSampleQueueSize: number
    videoSize: "720p" | "1080p" | "1440p" | "4k" | "native" | "custom"
    videoSizeCustom: {
        width: number
        height: number
    },
    fps: number
    playAudioLocal: boolean
    audioSampleQueueSize: number
    controllerConfig: ControllerConfig
}

export function defaultStreamSettings(): StreamSettings {
    return {
        bitrate: 5000,
        packetSize: 4096,
        fps: 60,
        videoSampleQueueSize: 20,
        videoSize: "1080p",
        videoSizeCustom: {
            width: 1920,
            height: 1080,
        },
        playAudioLocal: false,
        audioSampleQueueSize: 20,
        controllerConfig: {
            invertAB: false,
            invertXY: false
        }
    }
}

export function getLocalStreamSettings(): StreamSettings | null {
    let settings = null
    try {
        const settingsLoadedJson = localStorage.getItem("mlSettings")
        if (settingsLoadedJson == null) {
            return null
        }

        const settingsLoaded = JSON.parse(settingsLoadedJson)

        settings = defaultStreamSettings()
        Object.assign(settings, settingsLoaded)
    } catch (e) {
        localStorage.removeItem("mlSettings")
    }
    return settings
}
export function setLocalStreamSettings(settings?: StreamSettings) {
    localStorage.setItem("mlSettings", JSON.stringify(settings))
}

export type StreamSettingsChangeListener = (event: ComponentEvent<StreamSettingsComponent>) => void

export class StreamSettingsComponent implements Component {

    private divElement: HTMLDivElement = document.createElement("div")

    // TODO: move these to the input component
    private streamHeader: HTMLHeadingElement = document.createElement("h2")
    private bitrate: InputComponent
    private packetSize: InputComponent
    private fps: InputComponent

    private videoSize: SelectComponent
    private videoSizeWidth: InputComponent
    private videoSizeHeight: InputComponent

    private videoSampleQueueSize: InputComponent

    private audioHeader: HTMLHeadingElement = document.createElement("h2")
    private playAudioLocal: InputComponent

    private audioSampleQueueSize: InputComponent

    private controllerHeader: HTMLHeadingElement = document.createElement("h2")
    private controllerInvertAB: InputComponent
    private controllerInvertXY: InputComponent

    constructor(settings?: StreamSettings) {
        const defaultSettings = defaultStreamSettings()

        // Root div
        this.divElement.classList.add("settings")

        this.streamHeader.innerText = "Video"
        this.divElement.appendChild(this.streamHeader)

        // Bitrate
        this.bitrate = new InputComponent("bitrate", "number", "Bitrate", {
            defaultValue: defaultSettings.bitrate.toString(),
            value: settings?.bitrate?.toString(),
            step: "100",
        })
        this.bitrate.addChangeListener(this.onSettingsChange.bind(this))
        this.bitrate.mount(this.divElement)

        // Packet Size
        this.packetSize = new InputComponent("packetSize", "number", "Packet Size", {
            defaultValue: defaultSettings.packetSize.toString(),
            value: settings?.packetSize?.toString(),
            step: "100"
        })
        this.packetSize.addChangeListener(this.onSettingsChange.bind(this))
        this.packetSize.mount(this.divElement)

        // Fps
        this.fps = new InputComponent("fps", "number", "Fps", {
            defaultValue: defaultSettings.fps.toString(),
            value: settings?.fps?.toString(),
            step: "100"
        })
        this.fps.addChangeListener(this.onSettingsChange.bind(this))
        this.fps.mount(this.divElement)

        // Video Size
        this.videoSize = new SelectComponent("videoSize",
            [
                { value: "720p", name: "720p" },
                { value: "1080p", name: "1080p" },
                { value: "1440p", name: "1440p" },
                { value: "4k", name: "4k" },
                { value: "native", name: "native" },
                { value: "custom", name: "custom" }
            ],
            {
                displayName: "Video Size",
                preSelectedOption: settings?.videoSize || defaultSettings.videoSize
            }
        )
        this.videoSize.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSize.mount(this.divElement)

        this.videoSizeWidth = new InputComponent("videoSizeWidth", "number", "Video Width", {
            defaultValue: defaultSettings.videoSizeCustom.width.toString(),
            value: settings?.videoSizeCustom.width.toString()
        })
        this.videoSizeWidth.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSizeWidth.mount(this.divElement)

        this.videoSizeHeight = new InputComponent("videoSizeHeight", "number", "Video Height", {
            defaultValue: defaultSettings.videoSizeCustom.height.toString(),
            value: settings?.videoSizeCustom.height.toString()
        })
        this.videoSizeHeight.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSizeHeight.mount(this.divElement)

        // Video Sample Queue Size
        this.videoSampleQueueSize = new InputComponent("videoSampleQueueSize", "number", "Video Sample Queue Size", {
            defaultValue: defaultSettings.videoSampleQueueSize.toString(),
            value: settings?.videoSampleQueueSize?.toString()
        })
        this.videoSampleQueueSize.addChangeListener(this.onSettingsChange.bind(this))
        this.videoSampleQueueSize.mount(this.divElement)

        // Audio local
        this.audioHeader.innerText = "Audio"
        this.divElement.appendChild(this.audioHeader)

        this.playAudioLocal = new InputComponent("playAudioLocal", "checkbox", "Play Audio Local", {
            checked: settings?.playAudioLocal
        })
        this.playAudioLocal.addChangeListener(this.onSettingsChange.bind(this))
        this.playAudioLocal.mount(this.divElement)

        // Audio Sample Queue Size
        this.audioSampleQueueSize = new InputComponent("audioSampleQueueSize", "number", "Audio Sample Queue Size", {
            defaultValue: defaultSettings.audioSampleQueueSize.toString(),
            value: settings?.audioSampleQueueSize?.toString()
        })
        this.audioSampleQueueSize.addChangeListener(this.onSettingsChange.bind(this))
        this.audioSampleQueueSize.mount(this.divElement)

        // Controller
        this.controllerHeader.innerText = "Controller"
        this.divElement.appendChild(this.controllerHeader)

        this.controllerInvertAB = new InputComponent("controllerInvertAB", "checkbox", "Invert A and B", {
            checked: settings?.controllerConfig.invertAB
        })
        this.controllerInvertAB.addChangeListener(this.onSettingsChange.bind(this))
        this.controllerInvertAB.mount(this.divElement)

        this.controllerInvertXY = new InputComponent("controllerInvertXY", "checkbox", "Invert X and Y", {
            checked: settings?.controllerConfig.invertXY
        })
        this.controllerInvertXY.addChangeListener(this.onSettingsChange.bind(this))
        this.controllerInvertXY.mount(this.divElement)

        this.onSettingsChange()
    }

    private onSettingsChange() {
        if (this.videoSize.getValue() == "custom") {
            this.videoSizeWidth.setEnabled(true)
            this.videoSizeHeight.setEnabled(true)
        } else {
            this.videoSizeWidth.setEnabled(false)
            this.videoSizeHeight.setEnabled(false)
        }

        this.divElement.dispatchEvent(new ComponentEvent("ml-settingschange", this))
    }

    addChangeListener(listener: StreamSettingsChangeListener) {
        this.divElement.addEventListener("ml-settingschange", listener as any)
    }
    removeChangeListener(listener: StreamSettingsChangeListener) {
        this.divElement.removeEventListener("ml-settingschange", listener as any)
    }

    getStreamSettings(): StreamSettings {
        const settings = defaultStreamSettings()

        settings.bitrate = parseInt(this.bitrate.getValue())
        settings.packetSize = parseInt(this.packetSize.getValue())
        settings.fps = parseInt(this.fps.getValue())
        settings.videoSize = this.videoSize.getValue() as any
        settings.videoSizeCustom = {
            width: parseInt(this.videoSizeWidth.getValue()),
            height: parseInt(this.videoSizeHeight.getValue())
        }
        settings.videoSampleQueueSize = parseInt(this.videoSampleQueueSize.getValue())

        settings.playAudioLocal = this.playAudioLocal.isChecked()
        settings.audioSampleQueueSize = parseInt(this.audioSampleQueueSize.getValue())

        settings.controllerConfig.invertAB = this.controllerInvertAB.isChecked()
        settings.controllerConfig.invertXY = this.controllerInvertXY.isChecked()

        return settings
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}