import { Api, apiPutUser, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { setTouchContextMenuEnabled } from "./ios_right_click.js";
import { UserList } from "./component/user/list.js";
import { AddUserModal } from "./component/user/add_modal.js";
import { showModal } from "./component/modal/index.js";

async function startApp() {
    setTouchContextMenuEnabled(true)

    const api = await getApi()

    const rootElement = document.getElementById("root")
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const app = new AdminApp(api)
    app.mount(rootElement)

    app.forceFetch()
}

startApp()

class AdminApp implements Component {

    private api: Api

    private root = document.createElement("div")

    private userPanel = document.createElement("div")
    private addUserButton = document.createElement("button")
    private userSearch = document.createElement("input")
    private userList: UserList

    constructor(api: Api) {
        this.api = api

        // Select User Panel
        this.addUserButton.innerText = "Add User"
        this.addUserButton.addEventListener("click", async () => {
            const addUserModal = new AddUserModal()

            const user = await showModal(addUserModal)

            if (user) {
                await apiPutUser(this.api, user)
            }
        })
        this.userPanel.appendChild(this.addUserButton)

        this.userSearch.placeholder = "Search User"
        this.userSearch.type = "text"
        this.userSearch.addEventListener("input", this.onUserSearchChange.bind(this))
        this.userPanel.appendChild(this.userSearch)

        this.userList = new UserList(api)
        this.userList.mount(this.userPanel)

        this.root.appendChild(this.userPanel)
    }

    async forceFetch() {
        await this.userList.forceFetch(true)
    }

    private onUserSearchChange() {
        // TODO
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.root)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.root)
    }
}
