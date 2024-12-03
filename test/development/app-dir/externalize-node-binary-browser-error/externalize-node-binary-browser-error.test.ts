import { nextTestSetup } from 'e2e-utils'
import {
  hasRedbox,
  getRedboxDescription,
  getRedboxSource,
} from 'next-test-utils'
;(process.env.TURBOPACK ? describe.skip : describe)(
  'externalize-node-binary-browser-error',
  () => {
    const { next } = nextTestSetup({
      files: __dirname,
    })

    it('should error when import node binary on browser side', async () => {
      const browser = await next.browser('/')
      await hasRedbox(browser)
      const redbox = {
        description: await getRedboxDescription(browser),
        source: await getRedboxSource(browser),
      }

      expect(redbox.description).toBe('Failed to compile')
      expect(redbox.source).toMatchInlineSnapshot(`
        "./node_modules/foo-browser-import-binary/binary.node
        Error: Node.js binary module ./node_modules/foo-browser-import-binary/binary.node is not supported in the browser. Please only use the module on server side"
      `)
    })
  }
)
