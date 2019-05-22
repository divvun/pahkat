const yaml = require("yamljs")
const jref = require("json-ref-lite")
const fs = require("fs")
const path = require("path")
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

const classTmpl = ({
  baseUrl,
  className,
  description,
  props,
  example
}) => `
== ${baseUrl}${className}[${className}]

${description}

=== Properties

[cols="2a,4a,1a,5a"]
|===
| Key | Type | Required? | Description

${props.map(p => `
| ${baseUrl}prop/${p.key}[${p.key}]
| ${p.type}
| ${p.isRequired}
| ${p.description}
`.trim()).join("\n\n")}
|===

${example && `
=== Example

[source,json]
----
${example}
----
`.trim()}
`.trim()

const propTmpl = ({ propName, description, classes, baseUrl }) => `
== ${baseUrl}prop/${propName}[${propName}]

${description}

=== Used by

${classes.map(c => `
* ${baseUrl}${c}[${c}]
`.trim()).join("\n")}
`.trim()

const typeTmpl = ({ baseUrl, typeName, baseType, description, props }) => `
== ${baseUrl}type/${typeName}[${typeName}]

Base type: \`${baseType}\`

${description}

=== Used by

${props.map(p => `
* ${baseUrl}prop/${p}[${p}]
`.trim()).join("\n")}

`.trim()

const indexTmpl = (objs) => `
== Páhkat 0.1.0

=== Classes

${objs.map(o => `
* ${o.url}[${o.name}]
`.trim()).join("\n")}

`.trim()

function genProp(baseUrl, prop) {
  let x

  if (prop.type === "object" && prop.additionalProperties) {
    const vParam = prop.format ? `${baseUrl}type/${prop.format}[${prop.format} (string)]` : "string"
    return `Map of \`${prop.additionalProperties.type}\` to \`${vParam}\``
  } else if (prop.format) {
    x = {
      type: `${prop.format} (${prop.type})`,
      typeUrl: `type/${prop.format}`
    }
  } else if (prop.type === "object" && prop.key) {
    x = {
      type: prop.key,
      typeUrl: prop.key
    }
  } else if (prop.enum) {
    if (prop.enum.length === 1) {
      return `Constant: \`${prop.enum[0]}\``
    } else {
      return `\`${prop.type}\` where value in: \`${prop.enum.join(", ")}\``
    }
  }

  if (x) {
    if (x.typeUrl) {
      return `\`${baseUrl}${x.typeUrl}[${x.type}]\``
    }

    return `\`${x.type}\``
  }

  return `\`${prop.type}\``
}

function stripBackticks(item) {
  if (item.startsWith('`')) {
    item = item.substring(1)
  }

  if (item.endsWith('`')) {
    item = item.substring(0, item.length - 1)
  }
  
  return item
}

function genType(baseUrl, propKey, schema) {
  const prop = schema.properties[propKey]

  if (prop.oneOf) {
    return `One of:\n\n${prop.oneOf.map(p => genProp(baseUrl, p)).map(x => `* ${x}`).join("\n")}`
  }

  if (prop.type === "array") {
    return `${prop.items.enum ? "Set" : "Array"} of ${genProp(baseUrl, prop.items)}`
  }

  return genProp(baseUrl, prop)
}

async function main({ jsonLdPath, openApiPath }) {
  const oaTree = loadOpenApi(openApiPath)

  const basePath = __dirname + "/0.1.0"
  const baseUrl = "https://pahkat.org/0.1.0/"
  const lol = Object.keys(oaTree.components.schemas).map(k => {
    const schema = oaTree.components.schemas[k]

    return {
      fn: `${k}.adoc`,
      url: `${baseUrl}${k}`,
      name: k,
      body: classTmpl({
        baseUrl,
        className: k,
        description: schema.description || "No description provided.",
        props: Object.keys(schema.properties).map(k => {
          if (k.startsWith("@")) {
            return null
          }

          const prop = schema.properties[k]

          return {
            key: k,
            type: genType(baseUrl, k, schema),
            isRequired: schema.required.includes(k) ? "Yes" : "No",
            description: prop.description || "No description provided."
          }
        }).filter(x => x != null),
        example: schema.example ? schema.example.value : null
      })
    }
  })

  const propNightmares = Object.keys(oaTree.components.schemas).reduce((o, k) => {
    const schema = oaTree.components.schemas[k]

    Object.keys(schema.properties).forEach(prop => {
      if (prop.startsWith("@")) {
        return
      }

      if (!o[prop]) {
        o[prop] = new Set()
      }

      o[prop].add(k)
    })

    return o
  }, {})

  const typeFun = Object.keys(oaTree.components.schemas).reduce((o, k) => {
    const schema = oaTree.components.schemas[k]

    Object.keys(schema.properties).forEach(propKey => {
      if (propKey.startsWith("@")) {
        return
      }

      const prop = schema.properties[propKey]

      const format = (prop.items && prop.items.format) || prop.format
      const baseType = prop.additionalProperties ? "string" : (prop.items ? prop.items.type : prop.type)
      const formatInfo = oaTree.components["x-formats"]
        && oaTree.components["x-formats"][format]
        || {}

      if (format) {
        if (!o[format]) {
          o[format] = {
            typeName: format,
            baseType,
            description: (formatInfo.description || "No description provided.").trim(),
            props: new Set()
          }
        }

        o[format].props.add(propKey)
      }
    })

    return o
  }, {})

  const typeOut = Object.keys(typeFun).map(k => {
    const props = [...typeFun[k].props]
    props.sort()

    return {
      fn: `${k}.adoc`,
      body: typeTmpl(Object.assign(typeFun[k], { baseUrl, props }))
    }
  })

  const propOut = Object.keys(propNightmares).map(k => {
    const classes = [...propNightmares[k]]
    classes.sort()

    return {
      fn: `${k}.adoc`,
      body: propTmpl({
        propName: k,
        description: "No description provided.",
        classes,
        baseUrl
      })
    }
  })
  
  try { fs.mkdirSync(basePath) } catch(err) {}
  try { fs.mkdirSync(path.join(basePath, "prop")) } catch(err) {}
  try { fs.mkdirSync(path.join(basePath, "type")) } catch(err) {}
  lol.forEach(thing => {
    fs.writeFileSync(path.join(basePath, thing.fn), thing.body)
  })
  propOut.forEach(thing => {
    fs.writeFileSync(path.join(basePath, "prop", thing.fn), thing.body)
  })
  typeOut.forEach(thing => {
    fs.writeFileSync(path.join(basePath, "type", thing.fn), thing.body)
  })
  fs.writeFileSync(path.join(basePath, "_index.adoc"), indexTmpl(lol))
}

main({
    jsonLdPath: process.argv[2],
    openApiPath: process.argv[3]
  })
  .then(() => process.exit())
  .catch(err => {
    console.error(err)
    process.exit(1)
  })