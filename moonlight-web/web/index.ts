import { Api, getApi, apiPutHost } from "./api.js";
import { AddHostModal } from "./component/host/add_modal.js";
import { HostList } from "./component/host/list.js";
import { Component, ComponentEvent, ComponentHost } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { showModal } from "./component/modal.js";
import { setContextMenu } from "./component/context_menu.js";
import { GameList } from "./component/game/list.js";
import { Host } from "./component/host/index.js";
import { App } from "./api_bindings.js";

// TODO: error handler with popup

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const rootComponent = new MainApp(api)
    const root = new ComponentHost(rootElement, rootComponent)

    rootComponent.forceFetch()
}

console.log("starting app")
startApp()

class MainApp implements Component {
    private api: Api

    private divElement = document.createElement("div")

    private moonlightTextElement = document.createElement("h1")
    private hostAddButton: HTMLButtonElement = document.createElement("button")

    private currentDisplay: "hosts" | "games" = "hosts"
    private hostList: HostList
    private gameList: GameList | null = null

    constructor(api: Api) {
        this.api = api

        // Moonlight text
        this.moonlightTextElement.innerHTML = "Moonlight Web"

        // Host add button
        this.hostAddButton.innerText = "Add Host"
        this.hostAddButton.addEventListener("click", this.addHost.bind(this))

        // Host list
        this.hostList = new HostList(api)
        this.hostList.addHostOpenListener(this.onHostOpen.bind(this))

        // Append default elements
        this.divElement.appendChild(this.moonlightTextElement)
        this.divElement.appendChild(this.hostAddButton)
        this.hostList.mount(this.divElement)

        // Context Menu
        document.body.addEventListener("contextmenu", this.onContextMenu.bind(this))
    }

    private async addHost() {
        const modal = new AddHostModal()

        let host = await showModal(modal);
        if (host) {
            const newHost = await apiPutHost(this.api, host)

            if (newHost) {
                this.hostList.insertList(newHost.host_id, newHost)
            } else {
                showErrorPopup("couldn't add host")
            }
        } else {
            showErrorPopup("couldn't add host")
        }
    }

    private onContextMenu(event: MouseEvent) {
        const elements = [
            {
                name: "Reload",
                callback: this.forceFetch.bind(this)
            }
        ]

        setContextMenu(event, {
            elements
        })
    }

    private async onHostOpen(event: ComponentEvent<Host>) {
        const hostId = event.component.getHostId()

        this.setCurrentGames(hostId)
    }
    private setCurrentGames(hostId: number | null, cache?: Array<App>) {
        // We want to transition to host view
        if (hostId == null) {
            // We aren't currently in host view
            if (this.currentDisplay == "games") {
                this.gameList?.unmount(this.divElement)
                this.hostList.mount(this.divElement)
            }

            this.currentDisplay = "hosts"
            return
        }

        // If we're already in the correct state
        if (this.currentDisplay == "games" && this.gameList?.getHostId() == hostId) {
            return
        }

        // Unmount host view if present
        if (this.currentDisplay == "hosts") {
            this.hostList.unmount(this.divElement)
        }

        // Mount game view
        this.gameList = new GameList(this.api, hostId, cache ?? null)
        this.gameList.mount(this.divElement)

        this.currentDisplay = "games"
    }

    async forceFetch() {
        await Promise.all([
            this.hostList.forceFetch(),
            this.gameList?.forceFetch()
        ])

        if (this.currentDisplay == "games" &&
            this.gameList &&
            !this.hostList.getHost(this.gameList.getHostId())) {
            // The newly fetched list doesn't contain the hosts game view we're in -> go to hosts
            this.setCurrentGames(null)
        }
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}