import { Api, getApi } from "./api.js";
import { Component } from "./component";
import { showErrorPopup } from "./component/error";
import { setTouchContextMenuEnabled } from "./ios_right_click.js";

async function startApp() {
    setTouchContextMenuEnabled(true)

    const api = await getApi()

    const rootElement = document.getElementById("root")
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const app = new MainApp(api)
    app.mount(rootElement)

    app.forceFetch()
}

startApp()

class MainApp implements Component {

    private api: Api

    private root = document.createElement("div")

    constructor(api: Api) {
        this.api = api
    }

    async forceFetch() {
        // TODO
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.root)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.root)
    }
}