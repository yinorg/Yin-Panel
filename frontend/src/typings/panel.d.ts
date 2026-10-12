declare namespace Panel {

    interface Info extends ItemInfo {

    }

    interface ItemInfo extends Common.InfoBase {
        icon: ItemIcon |null
        title: string
        url: string
        sort?: number
        lanUrl?: string
        mobileUrl?: string
        description?: string
        openMethod: number
        itemIconGroupId ?:number
    }

    interface ItemIconGroup extends Common.InfoBase {
        icon?: string
        title?: string
        sort?:number
        parentId?: number | null
    }

    interface ItemIcon {
        itemType: number
        src ?: string
        fileName?: string
        text ?: string
        // bgColor ?: string
        backgroundColor ?: string
    }

    interface State {
        rightSiderCollapsed: boolean
        leftSiderCollapsed: boolean
        networkMode:PanelStateNetworkModeEnum | null
        panelConfig:panelConfig
    }

    interface panelConfig{
        backgroundImageSrc?:string
        wallpaperMode?:'theme' | 'custom' | 'library' | 'none'
        wallpaperKind?:'image' | 'video' | 'webBundle' | 'externalUrl'
        wallpaperSource?:string
        wallpaperPoster?:string
        /** The Core wallpaper library entry the user picked (see
         *  `wallpaperMode: 'library'`). Only an id: the layer resolves it against
         *  the library, so a stored choice survives a library rebuild. */
        wallpaperLibraryEntry?:string
        backgroundBlur?:number
        backgroundMaskNumber?:number
        iconStyle?:PanelPanelConfigStyleEnum
        iconTextColor?:string
        useThemeDefaults?:boolean
        iconTextInfoHideDescription?:boolean
        iconTextIconHideTitle?:boolean
        logoText?:string
        logoImageSrc?:string
        clockShowSecond?:boolean
        clockColor?:string
        searchBoxShow?:boolean
        searchBoxSearchIcon?:boolean
        marginTop?:number
        marginBottom?:number
        maxWidth?:number
        maxWidthUnit:string
        marginX?:number
        footerHtml?:string
        systemMonitorShow?:boolean
        systemMonitorShowTitle?:boolean
        netModeChangeButtonShow?:boolean
    }

    interface userConfig{
        panel:panelConfig
    }

    interface ItemIconSortRequest{
        sortItems:Common.SortItemRequest[]
        itemIconGroupId:number
    }
}
