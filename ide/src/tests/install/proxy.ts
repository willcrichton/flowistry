import * as mockttp from "mockttp";
import path from "path";

export const PROXY_PORT = 8000;

/**
 * Creates and starts a proxy server which routes requests made to zip files on Github
 * to a local zip file.
 * @param zip Path to the local zip which replaces the remote zip.
 * @param port Port to run the proxy server on.
 * @returns A new proxy server running on `port`.
 */
export const createProxy = async (zip: string, port = PROXY_PORT) => {
    // Create a proxy server with a self-signed HTTPS CA certificate
    const https = await mockttp.generateCACertificate();
    const server = mockttp.getLocal({ https });

    // Intercept only requests made for zips on Github
    server.anyRequest().thenPassThrough();
    server.get(/github.com.*.zip/).thenFromFile(200, path.resolve(zip));

    await server.start(port);
    return server;
};
