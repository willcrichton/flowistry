import { expect } from 'chai';
import { VSBrowser, Workbench, Notification } from "vscode-extension-tester";
import { MOCK_PROJECT_FILES, MOCK_PROJECT_DIRECTORY } from '../unit/util/constants';
import { tester } from "./index";

describe('Flowistry install tests', () => {
    before(async function() {
        this.timeout(10000);
        
        await VSBrowser.instance.openResources(MOCK_PROJECT_DIRECTORY);
        await VSBrowser.instance.openResources(MOCK_PROJECT_FILES.forward_slice);

        await tester.installFromMarketplace('wcrichton.flowistry');
    });

    it('Displays notification', async function() {
        this.timeout(150000);

        // Wait for a notification to appear with a timeout of 150 seconds
        expect(await VSBrowser.instance.driver.wait(() => {
            return notificationExists('Flowistry has successfully installed!');
        }, 150000));
    });
});

async function notificationExists(text: string): Promise<Notification | undefined> {
    let notifications: Notification[] = [];

    try {
        notifications = await new Workbench().getNotifications();
    }
    catch {
        console.warn('Workbench notifications detached');
    }

    for (const notification of notifications) {
        try {
            const message = await notification.getMessage();
            if (message.indexOf(text) >= 0) {
                return notification;
            }
        }
        catch {
            console.warn('Notification detached from page');
        }
    }
}
