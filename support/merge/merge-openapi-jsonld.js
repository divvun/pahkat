const yaml = require("yamljs")
const jref = require("json-ref-lite")
const fs = require("fs")
const { URL } = require("url")

function loadOpenApi(yamlPath) {
  const tree = yaml.parse(fs.readFileSync(yamlPath, "utf8"))

  // Make sure all schema items have their key inside them
  if (tree.components && tree.components.schemas) {
    Object.keys(tree.components.schemas).forEach(key => {
      const obj = tree.components.schemas[key]

      obj.key = key
    })
  }

  return jref.resolve(tree)
}

function getJsonLdTypeFromSchema(item) {
  if (item.properties["@type"] && item.properties["@type"].enum) {
    return item.properties["@type"].enum[0]
  } else {
    return item.key
  }
}

function generateRouteSkeleton(oaTree, ldContext) {
  const propsNs = ldContext.pf
  const mainNs = ldContext["@vocab"]

  const keys = Object.keys(ldContext).filter(k => !["@version", "@context", "@vocab", "pf"].includes(k))
  keys.filter(k => typeof ldContext[k] === "string")
    .forEach(k => {
      ldContext[k] = { "@id": ldContext[k] }
    })


  const results = keys.reduce((o, k) => {
    const id = ldContext[k]["@id"] || k
    o[k] = { name: k }
    if (id.startsWith("pf:")) {
      o[k].url = new URL(`${propsNs}${id.split(":").pop()}`)
    } else if (id.indexOf(":") > -1) {
      throw new Error("What namespace is this? " + id)
    } else {
      o[k].url = new URL(`${mainNs}${id}`)
    }

    return o
  }, {})

  console.log(results)

  const oaSchemas = Object.keys(oaTree.components.schemas).map(k => {
    const item = oaTree.components.schemas[k]
    const ldType = getJsonLdTypeFromSchema(item)
    const type = results[ldType]

    if (type == null) {
      throw new Error(`Missing JSON-LD type: ${ldType}`)
    }

    return {
      ld: type,
      schema: item
    }
  })

  const props = oaSchemas.reduce((o, x) => {
    const p = x.schema.properties
    Object.keys(p).forEach(k => {
      if (k.startsWith("@")) {
        return
      }

      const type = results[k] || ldContext[x.ld.name]["@context"][k]
      console.log(type)

      if (type == null) {
        throw new Error(`Missing JSON-LD type: ${k}`)
      }

      o[k] = {
        prop: true,
        ld: type,
        schema: p[k]
      }
    })
    return o
  }, {})
  return oaSchemas
}

async function main({ jsonLdPath, openApiPath }) {
  const oaTree = loadOpenApi(openApiPath)
  const ldContext = yaml.parse(fs.readFileSync(jsonLdPath, "utf8"))
  const skeleton = generateRouteSkeleton(oaTree, ldContext)
  console.log()
  // console.dir(oaTree.paths['/index.json']
  //   .get.responses[200]
  //   .content['application/json'].schema)
}

main({
    jsonLdPath: process.argv[2],
    openApiPath: process.argv[3]
  })
  .then(() => process.exit())
  .catch(err => {
    console.error(err.stack)
    process.exit(1)
  })