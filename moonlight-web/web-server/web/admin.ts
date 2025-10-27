import { Api, apiGetUser, apiPutUser, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { setTouchContextMenuEnabled } from "./ios_right_click.js";
import { UserList } from "./component/user/list.js";
import { AddUserModal } from "./component/user/add_modal.js";
import { showMessage, showModal } from "./component/modal/index.js";
import { buildUrl } from "./config_.js";

async function startApp() {
    setTouchContextMenuEnabled(true)

    const api = await getApi()

    checkPermissions(api)

    const rootElement = document.getElementById("root")
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const app = new AdminApp(api)
    app.mount(rootElement)

    app.forceFetch()
}

async function checkPermissions(api: Api) {
    const user = await apiGetUser(api, {
        name: null,
        user_id: null
    })

    if (user.role != "Admin") {
        await showMessage("You are not authorized to view this page!")

        window.location.href = buildUrl("/")
    }
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

            const userRequest = await showModal(addUserModal)

            if (userRequest) {
                const newUser = await apiPutUser(this.api, userRequest)

                this.userList.insertList(newUser.id, newUser)
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
