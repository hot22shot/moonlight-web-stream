import { Component } from "../index.js";
import { Api } from "../../api.js";
import { App } from "../../api_bindings.js";
import { setContextMenu } from "../context_menu.js";
import { showMessage } from "../modal.js";

export class Game implements Component {
    private api: Api

    private hostId: number
    private appId: number

    private divElement: HTMLDivElement = document.createElement("div")

    private imageElement: HTMLImageElement = document.createElement("img")
    private imageOverlayElement: HTMLImageElement = document.createElement("img")
    private nameElement: HTMLElement = document.createElement("p")

    private cache: App

    constructor(api: Api, hostId: number, appId: number, game: App) {
        this.api = api

        this.hostId = hostId
        this.appId = appId

        this.cache = game

        // Configure image
        this.imageElement.classList.add("app-image")
        this.imageElement.src = "TODO"

        // Configure image overlay
        this.imageOverlayElement.classList.add("app-image-overlay")

        // Configure name
        this.nameElement.classList.add("app-name")

        // Append elements
        this.divElement.appendChild(this.imageElement)
        this.divElement.appendChild(this.imageOverlayElement)
        this.divElement.appendChild(this.nameElement)

        this.divElement.addEventListener("click", this.onClick.bind(this))
        this.divElement.addEventListener("contextmenu", this.onContextMenu.bind(this))

        this.updateCache(game)
    }

    updateCache(cache: App) {
        this.cache = cache
    }

    private async onClick() {
        // TODO: start stream
    }

    private onContextMenu(event: MouseEvent) {
        const elements = []

        elements.push({
            name: "Show Details",
            callback: this.showDetails.bind(this),
        })

        setContextMenu(event, {
            elements
        })
    }

    private async showDetails() {
        const app = this.cache

        await showMessage(
            `Id: ${app.app_id}\n` +
            `Title: ${app.title}\n` +
            `HDR Supported: ${app.is_hdr_supported}\n`
        )
    }

    getHostId(): number {
        return this.hostId
    }
    getAppId(): number {
        return this.appId
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}