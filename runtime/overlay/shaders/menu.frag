#version 450

layout(binding = 0) uniform sampler2D menu;
layout(location = 0) in vec2 coord;
layout(location = 0) out vec4 color;

void main()
{
    color = texture(menu, coord);
}
